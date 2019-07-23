use null::null_fn_mut;
use std::cell::Cell;
use std::cell::RefCell;
use std::io::Result;
use timetravel::Context;

const STACK_SIZE_BYTES: usize = 2 * 1_024 * 1_024;

/// The result of `launch()`'ing a timed function.
///
/// The closure might have:
///  * completed and returned a value, *or*
///  * been preempted and now need to be explicitly `resume()`'d
pub enum Linger<T, F: FnMut() -> Option<T>> {
	Completion(T),
	Continuation(Continuation<T, F>),
}

// TODO: Store the current group, either here or in a separate variable.
#[derive(Default)]
struct Executing {
	checkpoint: Option<Context<Box<[u8]>>>,
	deadline: u64,
}

thread_local! {
	// TODO: Support nested timed functions by replacing with a stack.
	// TODO: Optimize by using an UnsafeCell.
	static EXECUTING: RefCell<Executing> = RefCell::default();
	static LAUNCHING: Cell<*mut (dyn FnMut() + Send)> = Cell::new(null_fn_mut());
}

/// Run `fun` with the specified time budget, in `us`econds.
///
/// If the budget is `0`, the timed function is initialized but not invoked; if it is `max_value()`,
/// it is run to completion.
pub fn launch<T: Send>(fun: impl FnOnce() -> T + Send, us: u64)
-> Result<Linger<T, impl FnMut() -> Option<T> + Send>> {
	use timetravel::makecontext;

	let mut task = None;
	makecontext(
		// TODO: Optimize by allocating the execution stacks from a pool.
		vec![0; STACK_SIZE_BYTES].into_boxed_slice(),
		|context| drop(task.replace(context)),
		schedule,
	)?;

	let mut result = None;
	let mut fun = Some(fun);
	let result = Cell::new(move || {
		if let Some(fun) = fun.take() {
			// TODO: Catch panics in the user-supplied closure?
			result.replace(fun())
		} else {
			debug_assert!(
				result.is_some(),
				"libinger: memoized closure called twice concurrently or thrice!",
			);
			result.take()
		}
	});
	LAUNCHING.with(|launching| {
		debug_assert!(
			launching.get().is_null(),
			"launch(): called twice concurrently from the same thread!",
		);

		let result = result.as_ptr();
		let result: *mut (dyn FnMut() -> Option<T> + Send) = result;
		let result: *mut (dyn FnMut() + Send) = result as _;
		launching.replace(result);
	});

	let mut state = Linger::Continuation(Continuation {
		task,
		result,
	});
	if us != 0 {
		resume(&mut state, us)?;
	}

	Ok(state)
}

/// Let `fun` continue running for the specified time budget, in `us`econds.
///
/// If the budget is `0`, this is a no-op; if it is `max_value()`, the timed function is run to
/// completion.  This function is idempotent once the timed function completes.
// TODO: Return the total time spent running?
pub fn resume<T>(fun: &mut Linger<T, impl FnMut() -> Option<T>>, us: u64) -> Result<()> {
	use lifetime::unbound_mut;
	use timetravel::restorecontext;
	use timetravel::sigsetcontext;

	if let Linger::Continuation(continuation) = fun {
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
					// TODO: Add in current wall-clock time (unless unlimited!).
					executing.deadline = us;

					let resume = executing.checkpoint.get_or_insert(pause);
					unsafe {
						unbound_mut(resume)
					}
				});

				let failure = sigsetcontext(resume);
				error.replace(failure.expect("resume(): continuation invalid!"));
			},
		).map_err(|or| or.expect("resume(): continuation contains invalid stack!"))?;
		if let Some(error) = error {
			Err(error)?;
		}

		if let Some(resume) = EXECUTING.with(|executing|
			executing.borrow_mut().checkpoint.take()
		) {
			continuation.task.replace(resume);
		} else {
			let completion = continuation.result.get_mut();
			let completion = completion().expect(
				"resume(): return value missing despite apparent completion!"
			);
			*fun = Linger::Completion(completion);
		}
	}

	Ok(())
}

fn schedule() {
	let fun = LAUNCHING.with(|launching| launching.replace(null_fn_mut()));
	debug_assert!(! fun.is_null(), "schedule() called from outside launch()!");

	let fun = unsafe {
		&mut *fun
	};
	// TODO: Enable preemption here.
	fun();
	// TODO: Disable preemption here.

	// The closure completed!  Drop the preemption checkpoint to inform resume().
	// TODO: This is safe, right (considering we still need the successor to return from here)?
	EXECUTING.with(|executing| {
		let mut executing = executing.borrow_mut();
		debug_assert!(
			executing.checkpoint.is_some(),
			"libinger: checkpoint disappeared before timed function completed!",
		);
		executing.checkpoint.take();
	});
}

/// Opaque representation of a timed function that has not yet returned.
// TODO: Make this non-Send!
// TODO: Add a field associating it with a group.
pub struct Continuation<T, F: FnMut() -> Option<T>> {
	task: Option<Context<Box<[u8]>>>,

	// When called, this function invokes the user-supplied closure and saves the result instead
	// of returning it.  On the immediately *following* invocation, it returns this value.  Note
	// that it is neither reentrant nor, consequently, AS-safe.  For this reason, care must be
	// taken not to attempt the second call until it is already known that the first completed.
	result: Cell<F>,
}

impl<T, F: FnMut() -> Option<T>> Drop for Continuation<T, F> {
	// TODO: Support aborting by reinitializing the namespace instead of resuming.
	fn drop(&mut self) {
		let mut fun = Linger::Continuation(Continuation {
			task: self.task.take(),
			result: Cell::new(self.result.get_mut()),
		});

		resume(&mut fun, u64::max_value()).expect("libinger: did not run to completion!");
	}
}
