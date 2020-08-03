use gotcha::Group;
use preemption::defer_preemption;
use preemption::disable_preemption;
use preemption::is_preemptible;
use preemption::thread_signal;
use reusable::ReusableSync;
use signal::Set;
use signal::Signal;
use signal::siginfo_t;
use stacks::DerefAdapter;
use super::QUANTUM_MICROSECS;
use std::cell::Cell;
use std::cell::RefCell;
use std::io::Result;
use std::os::raw::c_int;
use std::ptr::NonNull;
use std::thread::Result as ThdResult;
use tcb::ThreadControlBlock;
use timetravel::errno::errno;
use timetravel::Context;
use timetravel::HandlerContext;
use unfurl::Unfurl;

pub enum Linger<T, F: FnMut(*mut Option<ThdResult<T>>) + Send + ?Sized> {
	Completion(T),
	Continuation(Continuation<F>),
	Poison,
}

impl<T, F: FnMut(*mut Option<ThdResult<T>>) + Send + ?Sized> Unpin for Linger<T, F> {}

impl<T, F: FnMut(*mut Option<ThdResult<T>>) + Send + ?Sized> Linger<T, F> {
	pub fn is_completion(&self) -> bool {
		if let Linger::Completion(_) = self {
			true
		} else {
			false
		}
	}

	pub fn is_continuation(&self) -> bool {
		if let Linger::Continuation(_) = self {
			true
		} else {
			false
		}
	}

	pub fn yielded(&self) -> bool {
		if let Linger::Continuation(continuation) = self {
			continuation.stateful.yielded
		} else {
			false
		}
	}
}

impl<'a, T, F: FnMut(*mut Option<ThdResult<T>>) + Send + 'a> Linger<T, F> {
	#[doc(hidden)]
	pub fn erase(self) -> Linger<T, dyn FnMut(*mut Option<ThdResult<T>>) + Send + 'a> {
		use std::mem::MaybeUninit;

		if let Linger::Completion(this) = self {
			Linger::Completion(this)
		} else if let Linger::Continuation(this) = self {
			let this = MaybeUninit::new(this);
			let this = this.as_ptr();
			unsafe {
				let functional: *const _ = &(*this).functional;
				let stateful: *const _ = &(*this).stateful;
				let group: *const _ = &(*this).group;
				let tls: *const _ = &(*this).tls;

				Linger::Continuation(Continuation {
					functional: functional.read(),
					stateful: stateful.read(),
					group: group.read(),
					tls: tls.read(),
				})
			}
		} else {
			Linger::Poison
		}
	}
}

pub struct Continuation<T: ?Sized> {
	// First they called for the preemptible function to be executed, and I did not read the
	// argument because it was not present.  Then they called for the return value, and I did
	// not call the preemptible function because it was not present.  Then they called for me,
	// and I did not call or return because there was no one left to call and I had nothing left
	// to give.  (Only call this twice!)
	//
	// Because the whole Continuation might be moved between this function's preemption and its
	// resumption, we must heap allocate it so its captured environment has a stable address.
	functional: Box<T>,
	stateful: Task,
	group: ReusableSync<'static, Group>,
	tls: ThreadControlBlock,
}

unsafe impl<T: ?Sized> Send for Continuation<T> {}

impl<T: ?Sized> Drop for Continuation<T> {
	fn drop(&mut self) {
		debug_assert!(! is_preemptible(), "libinger: dropped from preemptible function");

		let group = &self.group;
		if self.stateful.errno.is_some() {
			// We're canceling a paused preemptible function.  Clean up the group!
			assert!(group.renew(), "libinger: failed to reinitialize library group");
		}
	}
}

#[derive(Default)]
struct Task {
	// Also indicates the state of execution.  If we have a Continuation instance, we know the
	// computatation cannot have completed earlier, so it must be in one of these states...
	//  * None on entry to resume() means it hasn't yet started running.
	//  * None at end of resume() means it has completed.
	//  * Some at either point means it timed out and is currently paused.
	errno: Option<c_int>,
	checkpoint: Option<Context<DerefAdapter<'static, ReusableSync<'static, Box<[u8]>>>>>,
	yielded: bool,
}

/// Run `fun` with the specified time budget, in `us`econds.
///
/// If the budget is `0`, the timed function is initialized but not invoked; if it is `max_value()`,
/// it is run to completion.
pub fn launch<T: Send>(fun: impl FnOnce() -> T + Send, us: u64)
-> Result<Linger<T, impl FnMut(*mut Option<ThdResult<T>>) + Send>> {
	use groups::assign_group;
	use std::panic::AssertUnwindSafe;
	use std::panic::catch_unwind;

	enum Completion<F, T> {
		Function(F),
		Return(T),
		Empty,
	}

	impl<F, T> Completion<F, T> {
		fn take(&mut self) -> Self {
			use std::mem::replace;

			replace(self, Completion::Empty)
		}
	}

	// Danger, W.R.!  Although in theory libgotcha ensures there's only one copy of our library,
	// exported parameterized functions are actually monomorphized into the *caller's* object
	// file!  This means that this function has an inconsistent view of the worldstate if called
	// from a preemptible function, but that any non-generic (name brand?) functions it calls
	// are guaranteed to run in the libgotcha's shared group.  To guard against heisenbugs
	// arising from the former case, we first assert that no one has attempted a nested call.
	debug_assert!(! is_preemptible(), "launch(): called from preemptible function");

	let mut fun = Completion::Function(AssertUnwindSafe (fun));
	let fun = Box::new(move |ret: *mut Option<ThdResult<T>>| {
		fun = match fun.take() {
		Completion::Function(fun) =>
			// We haven't yet moved the closure.  This means schedule() is invoking us
			// as a *nullary* function, implying ret is undefined and mustn't be used.
			Completion::Return(catch_unwind(fun)),
		Completion::Return(val) => {
			debug_assert!(! ret.is_null());
			let ret = unsafe {
				&mut *ret
			};
			ret.replace(val);
			Completion::Empty
		},
		Completion::Empty =>
			Completion::Empty,
		}
	});

	let checkpoint = setup_stack()?;
	let group = assign_group().expect("launch(): too many active timed functions");
	let mut linger = Linger::Continuation(Continuation {
		functional: fun,
		stateful: Task {
			errno: None,
			checkpoint,
			yielded: false,
		},
		group,
		tls: ThreadControlBlock::new(),
	});
	if us != 0 {
		resume(&mut linger, us)?;
	}
	Ok(linger)
}

thread_local! {
	static TLS: Cell<Option<ThreadControlBlock>> = Cell::default();
	static BOOTSTRAP: Cell<Option<(NonNull<(dyn FnMut() + Send)>, Group)>> = Cell::default();
	static TASK: RefCell<Task> = RefCell::default();
	static DEADLINE: Cell<u64> = Cell::default();
}

/// Let `fun` continue running for the specified time budget, in `us`econds.
///
/// If the budget is `0`, this is a no-op; if it is `max_value()`, the timed function is run to
/// completion.  This function is idempotent once the timed function completes.
pub fn resume<T>(fun: &mut Linger<T, impl FnMut(*mut Option<ThdResult<T>>) + Send + ?Sized>, us: u64)
-> Result<&mut Linger<T, impl FnMut(*mut Option<ThdResult<T>>) + Send + ?Sized>> {
	use std::panic::resume_unwind;

	// Danger, W.R!  The same disclaimer from launch() applies here.
	debug_assert!(! is_preemptible(), "resume(): called from preemptible function");

	if let Linger::Continuation(continuation) = fun {
		let task = &mut continuation.stateful;
		let group = *continuation.group;
		let tls = ThreadControlBlock::current()?;
		unsafe {
			continuation.tls.install()?;
		}
		TLS.with(|this_thread| this_thread.replace(tls.into()));

		// Are we launching this preemptible function for the first time?
		if task.errno.is_none() {
			let fun = &mut continuation.functional;
			BOOTSTRAP.with(|bootstrap| {
				// The schedule() function is polymorphic across "return" types, but
				// we expect a storage area appropriate for our own type.  To remove
				// the specialization from our signature, we reduce our arity by
				// casting away our parameter.  This is safe because schedule()
				// represents our first caller, so we know not to read the argument.
				let fun: *mut (dyn FnMut(_) + Send) = fun;
				let fun: *mut (dyn FnMut() + Send) = fun as _;
				let no_fun = bootstrap.replace(NonNull::new(fun).map(|fun|
					(fun, group)
				));
				debug_assert!(
					no_fun.is_none(),
					"resume(): bootstraps without an intervening schedule()",
				);
			});
		}

		DEADLINE.with(|deadline| deadline.replace(us));
		if switch_stack(task, group)? {
			let tls = TLS.with(|tls| tls.take());
			let tls = tls.expect("libinger: missing saved TCB at completion");
			unsafe {
				tls.install()?;
			}

			// The preemptible function finished (either ran to completion or panicked).
			// Since we know the closure is no longer running concurrently, it's now
			// safe to call it again to retrieve the return value.
			let mut retval = None;
			(continuation.functional)(&mut retval);

			match retval.expect("resume(): return value was already retrieved") {
				Ok(retval) => *fun = Linger::Completion(retval),
				Err(panic) => {
					*fun = Linger::Poison;
					resume_unwind(panic);
				},
			}
		}
	}

	Ok(fun)
}

/// Set up the oneshot execution stack.  Always returns a Some when things are Ok.
#[inline(never)]
fn setup_stack()
-> Result<Option<Context<DerefAdapter<'static, ReusableSync<'static, Box<[u8]>>>>>> {
	use stacks::alloc_stack;
	use timetravel::makecontext;

	let mut checkpoint = None;
	makecontext(
		DerefAdapter::from(alloc_stack()),
		|goto| drop(checkpoint.replace(goto)),
		schedule,
	)?;
	Ok(checkpoint)
}

/// Jump to the preemptible function, reenabling preemption if the function was previously paused.
/// Runs on the main execution stack.  Returns whether the function "finished" without a timeout.
#[inline(never)]
fn switch_stack(task: &mut Task, group: Group) -> Result<bool> {
	use gotcha::group_thread_set;
	use lifetime::unbound_mut;
	use signal::Operation;
	use signal::Sigset;
	use signal::pthread_sigmask;
	use timetravel::restorecontext;
	use timetravel::sigsetcontext;
	use preemption::thread_setup;

	if let Err(or) = thread_setup(preempt, QUANTUM_MICROSECS) {
		abort(&format!("switch_stack(): failure in thread_setup(): {}", or));
	}

	let mut error = None;
	restorecontext(
		task.checkpoint.take().expect("switch_stack(): continuation is missing"),
		|pause| {
			let resume = TASK.with(|task| {
				let mut task = task.borrow_mut();
				debug_assert!(
					task.checkpoint.is_none(),
					"switch_stack(): this continuation would nest?!"
				);

				let resume = task.checkpoint.get_or_insert(pause);
				unsafe {
					unbound_mut(resume)
				}
			});

			// Are we resuming a paused preemptible function?
			if let Some(erryes) = task.errno {
				// Do everything to enable preemption short of unblocking the
				// signal, which will be don atomically by sigsetcontext() as it
				// jumps into the continuation.
				let mut old = Sigset::empty();
				let mut new = Sigset::empty();
				let sig = thread_signal();
				let sig = unsafe {
					sig.unfurl()
				};
				new.add(sig);
				if let Err(or) = pthread_sigmask(
					Operation::Block,
					&new,
					Some(&mut old),
				) {
					error.replace(or);
				}
				old.del(sig);
				*resume.mask() = old;

				// The clock is ticking from this "start" point.
				stamp();

				// The order of these two lines, with respect to both each other and
				// the rest of the program, is very important.  We need to switch to
				// the new group before we restore errno so we get the right one,
				// and there cannot be any standard library calls between its
				// restoration and the call to sigsetcontext().
				group_thread_set!(group);
				*errno() = erryes;
			}

			// No library calls may be made before this one!
			let failure = sigsetcontext(resume);
			error.replace(failure.expect("resume(): continuation is invalid"));
		},
	)?;
	if let Some(error) = error {
		Err(error)?;
	}

	let descheduled = TASK.with(|task| task.replace(Task::default()));
	let preempted = descheduled.errno.is_some();
	if preempted {
		*task = descheduled;
	} else {
		// Prevent namespace reinitialization on drop of Continuation containing the Task.
		task.errno.take();
	}
	Ok(! preempted)
}

/// Enable preemption and call the preemptible function.  Runs on the oneshot execution stack.
fn schedule() {
	use preemption::enable_preemption;

	let (mut fun, group) = BOOTSTRAP.with(|bootstrap| bootstrap.take()).unwrap_or_else(||
		abort("schedule(): called without bootstrapping")
	);
	let fun = unsafe {
		fun.as_mut()
	};
	stamp();
	enable_preemption(group.into());
	fun();
	disable_preemption();

	// It's important that we haven't saved any errno, since we'll check it to determine whether
	// the preemptible function ran to completion.
	debug_assert!(
		TASK.with(|task| task.borrow().errno.is_none()),
		"schedule(): finished leaving errno",
	);
}

/// Signal handler that pauses the preemptible function on timeout.  Runs on the oneshot stack.
extern fn preempt(no: Signal, _: Option<&siginfo_t>, uc: Option<&mut HandlerContext>) {
	use timetravel::Swap;

	let erryes = *errno();
	let uc = unsafe {
		uc.unfurl()
	};
	let relevant = thread_signal().map(|signal| no == signal).unwrap_or(false);
	if relevant && is_preemptible() {
		let deadline = DEADLINE.with(|deadline| deadline.get());
		if nsnow() >= deadline {
			TASK.with(|task| {
				// It's time to pause the function.  We need to save its state.
				let mut task = task.borrow_mut();

				// Did it cooperatively yield instead of being preempted?
				task.yielded = deadline == 0;

				// Configure us to return into the checkpoint for its call site.
				let checkpoint = task.checkpoint.as_mut();
				let checkpoint = unsafe {
					checkpoint.unfurl()
				};
				checkpoint.swap(uc);

				// Instead of restoring errno, save it for if and when we resume.
				task.errno.replace(erryes);

				// Block this signal and disable preemption.
				uc.uc_sigmask.add(no);
				disable_preemption();

				// Restore the thread's original thread-control block.
				let tls = TLS.with(|tls| tls.take());
				let tls = tls.expect("libinger: missing saved TCB during preemption");
				unsafe {
					tls.install().expect("libinger: failed to restore TCB");
				}
			});
		} else {
			*errno() = erryes;
		}
	} else {
		if relevant {
			// The timed function has called into a nonpreemptible library function.
			// We'll need to intercept it immediately upon the function's return.
			defer_preemption();
		}

		// Block this signal so it doesn't disturb us again.
		uc.uc_sigmask.add(no);

		*errno() = erryes;
	}
}

/// Bump the deadline forward by the current wall-clock time, unless the timeout is unlimited.
fn stamp() {
	DEADLINE.with(|deadline|
		deadline.replace(deadline.get().checked_mul(1_000).map(|timeout|
			nsnow() + timeout
		).unwrap_or(deadline.get()))
	);
}

/// Immediately yield the calling preemptible function.
#[inline(never)]
pub fn pause() {
	// Note that this function is not parameterized, so it is itself nonpreemptible.  This is
	// key because we need to be able to request preemption atomically (so we only get one).
	DEADLINE.with(|deadline| deadline.take());

	// Get preempted when we return.
	defer_preemption();
}

/// Read the current wall-clock time, in nanoseconds.
#[doc(hidden)]
pub fn nsnow() -> u64 {
	use std::time::UNIX_EPOCH;
	use std::time::SystemTime;

	let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("libinger: wall clock error");
	let mut sum = now.subsec_nanos().into();
	sum += now.as_secs() * 1_000_000_000;
	sum
}

fn abort(err: &str) -> ! {
	use std::process::abort;

	let abort: fn() -> _ = abort;
	eprintln!("{}", err);
	abort()
}
