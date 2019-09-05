use gotcha::Group;
use preemption::defer_preemption;
use preemption::disable_preemption;
use preemption::thread_signal;
use reusable::ReusableSync;
use signal::Set;
use signal::Signal;
use signal::siginfo_t;
use std::cell::Cell;
use std::cell::RefCell;
use std::io::Result;
use std::os::raw::c_int;
use std::ptr::NonNull;
use std::thread::Result as ThdResult;
use timetravel::errno::errno;
use timetravel::Context;
use timetravel::HandlerContext;

const QUANTUM_MICROSECS: u64  = 15;
const STACK_SIZE_BYTES: usize = 2 * 1_024 * 1_024;

/// The result of `launch()`'ing a timed function.
///
/// The closure might have:
///  * completed and returned a value, *or*
///  * been preempted and now need to be explicitly `resume()`'d
// The only time this can contain a None is during a call to Linger::into() -> Option<T>.
pub struct Linger<T, F: FnMut(*mut Option<ThdResult<T>>)> (Option<TaggedLinger<T, F>>);

impl<T, F: FnMut(*mut Option<ThdResult<T>>)> Linger<T, F> {
	/// Indicates whether this closure has completed and returned a value.
	///
	/// This check is `true` if and only if conversion `into()` an `Option` will yield a `Some`;
	/// if you need to use the function's return value, perform the conversion instead.
	pub fn is_completion(&self) -> bool {
		let this: Option<&T> = self.into();
		this.is_some()
	}

	/// Indicates whether this closure has been preempted before it could complete.
	pub fn is_continuation(&self) -> bool {
		let this: Option<&T> = self.into();
		this.is_none()
	}
}

impl<'a, T, F: FnMut(*mut Option<ThdResult<T>>)> Into<Option<&'a T>> for &'a Linger<T, F> {
	fn into(self) -> Option<&'a T> {
		let Linger (this) = self;
		if let TaggedLinger::Completion(this) = this.as_ref().unwrap() {
			Some(this)
		} else {
			None
		}
	}
}

impl<'a, T, F: FnMut(*mut Option<ThdResult<T>>)> Into<Option<&'a mut T>> for &'a mut Linger<T, F> {
	fn into(self) -> Option<&'a mut T> {
		let Linger (this) = self;
		if let TaggedLinger::Completion(this) = this.as_mut().unwrap() {
			Some(this)
		} else {
			None
		}
	}
}

impl<T, F: FnMut(*mut Option<ThdResult<T>>)> Into<Option<T>> for Linger<T, F> {
	fn into(mut self) -> Option<T> {
		let Self (this) = &mut self;
		let that = this.take().unwrap();
		if let TaggedLinger::Completion(that) = that {
			Some(that)
		} else {
			// Put Humpty Dumpty back together again so it can be drop()'d!
			this.replace(that);

			None
		}
	}
}

impl<T, F: FnMut(*mut Option<ThdResult<T>>)> Drop for Linger<T, F> {
	// TODO: Support aborting by reinitializing the namespace instead of resuming.
	fn drop(&mut self) {
		use std::thread::panicking;

		if let Self (Some(TaggedLinger::Continuation(this))) = self {
			if let Some(group) = this.group.take() {
				assert!(
					group.renew(),
					"libgotcha: failed to reinitialize group for reuse",
				);
			} else {
				// The fact that we're currently panicking might mean that we failed
				// to assign a new group identifier to the continuation.
				assert!(panicking());
			}
		}
	}
}

// TODO: Store the current group, either here or in a separate variable.
#[derive(Default)]
struct Executing {
	checkpoint: Option<Context<Box<[u8]>>>,
	preempted: Option<c_int>,
	deadline: u64,
}

thread_local! {
	// TODO: Support nested timed functions by replacing with a stack.
	// TODO: Optimize by using an UnsafeCell.
	static EXECUTING: RefCell<Executing> = RefCell::default();
	static LAUNCHING: Cell<Option<(NonNull<dyn FnMut() + Send>, Group)>> = Cell::default();
}

/// Run `fun` with the specified time budget, in `us`econds.
///
/// If the budget is `0`, the timed function is initialized but not invoked; if it is `max_value()`,
/// it is run to completion.
// TODO: Because this (and resume()) are parameterized on types, their monomorphized code is present
//       in *client* implementations and therefore not subject to library group trampolining.  This
//       will introduce concurrency bugs during nested launch() invocations; one way to address it
//       would be to implement them using polymorphic functions using dynamic dispatch internally.
pub fn launch<T: Send>(fun: impl FnOnce() -> T + Send, us: u64)
-> Result<Linger<T, impl FnMut(*mut Option<ThdResult<T>>) + Send>> {
	use groups::assign_group;
	use std::hint::unreachable_unchecked;
	use std::panic::AssertUnwindSafe;
	use std::panic::catch_unwind;
	use timetravel::makecontext;

	let mut task = None;
	thread_setup()?;
	makecontext(
		// TODO: Optimize by allocating the execution stacks from a pool.
		vec![0; STACK_SIZE_BYTES].into_boxed_slice(),
		|context| drop(task.replace(context)),
		schedule,
	)?;

	let mut result = None;
	let mut fun = Some(AssertUnwindSafe (fun));
	let result = Box::new(move |ret: *mut Option<ThdResult<T>>| {
		if let Some(fun) = fun.take() {
			result.replace(catch_unwind(fun));
		} else {
			debug_assert!(
				! ret.is_null(),
				"libinger: memoized closure re-called with null output argument",
			);

			let result = result.take();
			unsafe {
				ret.replace(result);
			}
		}
	});

	let mut state = Linger (None);
	let Linger (continuation) = &mut state;
	let continuation = continuation.get_or_insert(TaggedLinger::Continuation(Continuation {
		group: None,
		task,
		errno: 0,
		result,
	}));
	let continuation = if let TaggedLinger::Continuation(continuation) = continuation {
		continuation
	} else {
		unsafe {
			unreachable_unchecked()
		}
	};
	let group = assign_group().expect("launch(): too many active timed functions");
	LAUNCHING.with(|launching| {
		debug_assert!(
			launching.get().is_none(),
			"launch(): called twice concurrently from the same thread!",
		);

		let result: *mut (dyn FnMut(*mut Option<ThdResult<T>>) + Send) = &mut continuation.result;
		let result: *mut (dyn FnMut() + Send) = result as _;
		launching.replace(NonNull::new(result).map(|fun| (fun, *group)));
	});
	if us == 0 {
		// If the user doesn't want us to run their closure yet, ask schedule() to preempt
		// the moment it enables preemptive execution.
		defer_preemption();
	}
	resume(&mut state, us)?;

	if let Linger (Some(TaggedLinger::Continuation(continuation))) = &mut state {
		continuation.group.replace(group);
	}

	Ok(state)
}

/// Let `fun` continue running for the specified time budget, in `us`econds.
///
/// If the budget is `0`, this is a no-op; if it is `max_value()`, the timed function is run to
/// completion.  This function is idempotent once the timed function completes.
// TODO: Return the total time spent running?
pub fn resume<T>(fun: &mut Linger<T, impl FnMut(*mut Option<ThdResult<T>>)>, us: u64)
-> Result<&mut Linger<T, impl FnMut(*mut Option<ThdResult<T>>)>> {
	use gotcha::group_thread_set;
	use lifetime::unbound_mut;
	use signal::Operation;
	use signal::Sigset;
	use signal::pthread_sigmask;
	use std::panic::resume_unwind;
	use timetravel::restorecontext;
	use timetravel::sigsetcontext;
	use unfurl::Unfurl;

	let Linger (tfun) = fun;
	if let TaggedLinger::Continuation(continuation) = tfun.as_mut().unwrap() {
		let mut error = None;
		restorecontext(
			continuation.task.take().expect("resume(): continuation missing!"),
			|pause| {
				let resume = EXECUTING.with(|executing| {
					let mut executing = executing.borrow_mut();
					debug_assert!(
						executing.checkpoint.is_none(),
						"libinger: timed function tried to call launch()!",
					);

					// Add current wall-clock time, unless duration's unlimited.
					executing.deadline = us
						.checked_mul(1_000).map(|us| nsnow() + us)
						.unwrap_or(us);

					let resume = executing.checkpoint.get_or_insert(pause);
					unsafe {
						unbound_mut(resume)
					}
				});

				if let Some(group) = &continuation.group {
					// We're resuming a continuation that has been preempted.
					// Do everything to enable preemption short of unblocking
					// the signal, which will be done atomically as we jump into
					// the continuation.
					let mut old = Sigset::empty();
					let mut new = Sigset::empty();
					let sig = thread_signal();
					let sig = unsafe {
						sig.unfurl()
					};
					new.add(sig);
					drop(pthread_sigmask(Operation::Block, &new, Some(&mut old)));
					old.del(sig);
					*resume.mask() = old;
					group_thread_set!(**group);
				}
				*errno() = continuation.errno;

				// TODO: Make sigsetcontext() restore the signal mask (in contrast
				//       to setcontext()) so we can't get preempted until it's done.
				let failure = sigsetcontext(resume);
				error.replace(failure.expect("resume(): continuation invalid!"));
			},
		).map_err(|or| or.expect("resume(): continuation contains invalid stack!"))?;
		if let Some(error) = error {
			Err(error)?;
		}

		let executing = EXECUTING.with(|executing| executing.replace(Executing::default()));
		if let Some(errno) = executing.preempted {
			let checkpoint = executing.checkpoint
				.expect("resume(): checkpoint missing following preemption!");
			continuation.task.replace(checkpoint);
			continuation.errno = errno;
		} else {
			let mut result = None;
			(continuation.result)(&mut result);

			match result.expect("resume(): return value missing on completion!") {
				Ok(result) => drop(tfun.replace(TaggedLinger::Completion(result))),
				Err(panic) => {
					tfun.take();
					resume_unwind(panic);
				},
			}
		}
	}

	Ok(fun)
}

// TODO: Remove this wrapper if and when we solve the trampolining boundary issue.
fn thread_setup() -> Result<()> {
	use preemption::thread_setup;

	thread_setup(preempt, QUANTUM_MICROSECS)
}

fn schedule() {
	use preemption::enable_preemption;

	let (mut fun, group) = LAUNCHING.with(|launching| launching.take())
		.expect("libinger: schedule() called from outside launch()!");
	let fun = unsafe {
		fun.as_mut()
	};
	enable_preemption(group.into());
	fun();
	disable_preemption();
}

extern fn preempt(no: Signal, _: Option<&siginfo_t>, uc: Option<&mut HandlerContext>) {
	use preemption::is_preemptible;
	use timetravel::Swap;
	use unfurl::Unfurl;

	let erryes = *errno();
	let uc = unsafe {
		uc.unfurl()
	};
	let relevant = thread_signal().map(|signal| no == signal).unwrap_or(false);
	if relevant && is_preemptible() {
		EXECUTING.with(|executing| {
			let mut executing = executing.borrow_mut();
			if nsnow() >= executing.deadline {
				// We're preempting the timed function.  Store the C library's error
				// number and configure us to return into the stored checkpoint.
				let checkpoint = executing.checkpoint.as_mut();
				let checkpoint = unsafe {
					checkpoint.unfurl()
				};
				checkpoint.swap(uc);
				executing.preempted.replace(erryes);

				// Disable preemption and prepare to return into libinger code.
				uc.uc_sigmask.add(no);
				disable_preemption();
			}
		});
	} else {
		if relevant {
			// The timed function has called into a nonpreemptible library function.
			// We'll need to intercept it immediately upon the function's return.
			defer_preemption();
		}

		// Block this signal so it doesn't disturb us again.
		uc.uc_sigmask.add(no);
	}

	*errno() = erryes;
}

enum TaggedLinger<T, F> {
	Completion(T),
	Continuation(Continuation<F>),
}

/// Opaque representation of a timed function that has not yet returned.
// TODO: Make this non-Send!
// TODO: Add a field associating it with a group.
struct Continuation<T> {
	group: Option<ReusableSync<'static, Group>>,
	task: Option<Context<Box<[u8]>>>,
	errno: c_int,

	// When called, this function invokes the user-supplied closure and saves the result instead
	// of returning it.  On the immediately *following* invocation, it returns this value.  Note
	// that it is neither reentrant nor, consequently, AS-safe.  For this reason, care must be
	// taken not to attempt the second call until it is already known that the first completed.
	// TODO: Eliminate the second allocation by somehow merging this into the ancillary stack?
	result: Box<T>,
}

#[doc(hidden)]
pub fn nsnow() -> u64 {
	use std::time::UNIX_EPOCH;
	use std::time::SystemTime;

	let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("libinger: wall clock error");
	let mut sum = now.subsec_nanos().into();
	sum += now.as_secs() * 1_000_000_000;
	sum
}
