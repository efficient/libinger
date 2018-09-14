use continuation::CallStack;
use continuation::CallStackLock;
use continuation::UntypedContinuation;
use libc::SA_RESTART;
use libc::SA_SIGINFO;
use libc::__errno_location;
use libc::siginfo_t;
use libc::suseconds_t;
use libc::time_t;
use signal::Action;
use signal::Set;
use signal::Sigaction;
use signal::Signal;
use signal::Sigset;
use signal::sigaction;
use std::cell::Cell;
use std::cmp::max;
use std::cmp::min;
pub use std::io::Error;
use std::io::Result;
use std::mem::uninitialized;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::panic::resume_unwind;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::atomic::ATOMIC_USIZE_INIT;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::ONCE_INIT;
use std::sync::Once;
use std::thread::Result as PanicResult;
use std::time::UNIX_EPOCH;
use std::time::SystemTime;
use time::Timer;
use time::itimerval;
use time::setitimer;
use time::timeval;
use timetravel::HandlerContext;
use timetravel::Swap;
use timetravel::makecontext;
use timetravel::restorecontext;
use timetravel::setcontext;
use timetravel::sigsetcontext;

const STACK_SIZE_BYTES: usize = 2 * 1_024 * 1_024;
const TIME_QUANTUM_DIVISOR: u64 = 3;

static QUANTUM: AtomicUsize = ATOMIC_USIZE_INIT;

thread_local! {
	// In the context of preemption enablement, None means it is disabled: any preemptions that
	// occur will simply be ignored.  In the context of continuation packaging, None means to
	// save only the frame on top of the call stack.
	static EARLIEST: Cell<Option<usize>> = Cell::new(None);
}

enum LaunchResume<T, F: FnMut() -> T> {
	Launch(F),
	Resume((UntypedContinuation, Vec<UntypedContinuation>)),
}

pub struct Continuation<T, F: FnMut() -> T> {
	function: LaunchResume<T, F>,
	result: Weak<Cell<PanicResult<T>>>,
}

pub enum Linger<T, F: FnMut() -> T> {
	Completion(T),
	Continuation(Continuation<T, F>),
}

impl<T, F: FnMut() -> T> Linger<T, F> {
	pub fn is_completion(&self) -> bool {
		if let &Linger::Completion(_) = self {
			true
		} else {
			false
		}
	}

	pub fn is_continuation(&self) -> bool {
		if let &Linger::Continuation(_) = self {
			true
		} else {
			false
		}
	}
}

pub fn launch<T: 'static, F: 'static + FnMut() -> T>(fun: F, us: u64) -> Result<Linger<T, F>> {
	use libc::ucontext_t;
	use std::mem::transmute;

	static INIT_HANDLER: Once = ONCE_INIT;
	INIT_HANDLER.call_once(|| {
		let preempt: extern "C" fn(Signal, Option<&siginfo_t>, Option<&mut HandlerContext>) = preempt;
		let preempt: extern "C" fn(Signal, Option<&siginfo_t>, Option<&mut ucontext_t>) = unsafe {
			transmute(preempt)
		};
		let handler = Sigaction::new(preempt, Sigset::empty(), SA_SIGINFO | SA_RESTART);
		sigaction(Signal::Alarm, &handler, None).unwrap();
	});

	resume(Continuation {
		function: LaunchResume::Launch(fun),
		result: Weak::new(),
	}, us)
}

pub fn resume<T: 'static, F: 'static + FnMut() -> T>(funs: Continuation<T, F>, us: u64) -> Result<Linger<T, F>> {
	let mut res = funs.result;
	match funs.function {
		LaunchResume::Launch(mut thunk) => {
			let mut err = None;
			let mut call_stack = CallStack::lock();
			let stack = vec![0u8; STACK_SIZE_BYTES].into_boxed_slice();
			makecontext(
				stack,
				|call_gate| {
					let ult: Rc<Cell<PanicResult<T>>> = Rc::new(Cell::new(Err(Box::new(()))));
					res = Rc::downgrade(&ult);

					let thunk = move || {
						let res = catch_unwind(AssertUnwindSafe (&mut thunk));
						EARLIEST.with(|earliest| earliest.set(None));
						ult.set(res);
					};
					let frame = UntypedContinuation::new(thunk, us, call_gate);
					call_stack.push(frame);

					let call_gate: *const _ = &call_stack[call_stack.len() - 1].pause_resume;
					drop(call_stack);
					err = Some(setcontext(call_gate));
				},
				preemptor,
			)?;

			if let Some(err) = err {
				if let Some(err) = err {
					Err(err)?;
				}
				panic!("launch(): Call gate expired before it was ever used!");
			}
		},
		LaunchResume::Resume((mut cont, inuations)) => {
			let mut call_stack = CallStack::lock();
			let checkpoint = cont.pause_resume;
			cont.pause_resume = unsafe {
				uninitialized()
			};
			restorecontext(
				checkpoint,
				|checkpoint| {
					cont.nested = Some(inuations);
					cont.pause_resume = checkpoint;
					call_stack.push(cont);
					drop(call_stack);
					preemptor();
				},
			).map_err(|or| if let Some(or) = or {
				or
			} else {
				panic!("resume(): Checkpoint could not be restored!")
			})?;
		},
	}

	let mut call_stack = CallStack::lock();
	let index = EARLIEST.with(|earliest| earliest.take())
		.map(|earliest| earliest + 1).unwrap_or(call_stack.len());
	let mut tail = call_stack.split_off(index);
	let head = call_stack.pop().unwrap();
	// Handle must be destroyed before call_stack because concurrency is enabled at that point!
	drop(index);

	let quantum = call_stack.iter()
		.map(|frame| frame.time_limit).min()
		.unwrap_or(0) / TIME_QUANTUM_DIVISOR;
	maybe_update_quantum(quantum);
	search_update_earliest(&call_stack);

	Ok(if head.time_out != 0 {
		debug_assert!(tail.is_empty());

		let res = res.upgrade().unwrap();
		drop(head);
		let res = Rc::try_unwrap(res).ok().unwrap().into_inner()
			.unwrap_or_else(|panic| resume_unwind(panic));

		Linger::Completion(res)
	} else {
		let ts = nsnow();
		for frame in &mut tail {
			frame.time_out -= min(ts, frame.time_out);
		}

		Linger::Continuation(Continuation {
			function: LaunchResume::Resume((head, tail)),
			result: res,
		})
	})
}

fn preemptor() {
	// If we got preempted before taking this lock, we would never be able to resume() this
	// invocation's continuation because we wouldn't yet have captured the address of the
	// original thunk closure.  However, one cannot actually construct such a scenario:
	// In the case where we are currently launch()'ing the only in-flight preemptible function,
	// this thread's EARLIEST is set to None, so it will ignore any SIGALRM that occurs here.
	// In the case where we are launch()'ing or resume()'ing one preemptible function from
	// within another, we *can* be preempted here; however, EARLIEST will not yet have been set
	// to reflect this invocation, so our progress will be recorded within a parent call_stack
	// frame's continuation.  If said continuation is later resumed, we'll continue from here;
	// otherwise, we'll be cleaned up along with it, and resume()'ing will no longer be possible
	// in any case.
	let mut call_stack = CallStack::lock();
	let index = call_stack.len() - 1;

	let thunk;
	let checkpoint: *mut _;
	let nested;
	let time_out;
	{
		let frame = call_stack.last_mut().unwrap();
		let ptr: *mut _ = &mut *frame.thunk;
		thunk = unsafe {
			&mut *ptr
		};
		checkpoint = &mut frame.pause_resume;
		nested = frame.nested.take();

		let time_limit = frame.time_limit;
		maybe_update_quantum(max(time_limit / TIME_QUANTUM_DIVISOR, 1));

		time_out = nsnow() + time_limit * 1_000;
		frame.time_out = time_out;
	}

	EARLIEST.with(|earliest| {
		let earliest_out = earliest.get()
			.map(|index| call_stack[index].time_out)
			.unwrap_or(u64::max_value());
		if time_out < earliest_out {
			earliest.set(Some(index));
		}
	});

	if let Some(mut nested) = nested {
		if ! nested.is_empty() {
			let quantum = nested.iter()
				.map(|frame| frame.time_limit).min()
				.unwrap_or(u64::max_value()) / TIME_QUANTUM_DIVISOR;
			maybe_update_quantum(quantum);

			let ts = nsnow();
			for frame in nested.iter_mut() {
				frame.time_out += ts;
			}

			call_stack.append(&mut nested);
			search_update_earliest(&call_stack);
		}

		drop(nested);
		drop(call_stack);
		panic!(format!("resume(): Failed to restore checkpoint, error {:?}", sigsetcontext(checkpoint)));
	} else {
		drop(call_stack);
		thunk();
	}
}

extern "C" fn preempt(_: Signal, _: Option<&siginfo_t>, sigctxt: Option<&mut HandlerContext>) {
	let errno = unsafe {
		__errno_location().as_mut()
	}.unwrap();
	let errnot = *errno;

	if let Ok(mut call_stack) = unsafe {
		CallStack::preempt()
	} {
		if let Some(index) = EARLIEST.with(|earliest| earliest.get()) {
			{
				let frame = &mut call_stack[index];
				if nsnow() < frame.time_out {
					return;
				}
				frame.time_out = 0;

				let mut sigctxt = sigctxt.unwrap();
				frame.pause_resume.swap(&mut sigctxt);
			}

			// No more preemptions until resume() has finished bundling up the
			// continuation, at which point they will be automatically reenabled
			call_stack.lock();
		}
	}

	*errno = errnot;
}

fn maybe_update_quantum(proposed: u64) -> bool {
	use std::mem::swap;

	let proposed = proposed as usize;
	let mut current = QUANTUM.load(Ordering::Relaxed);

	while {
		if proposed < current || current == 0 {
			let interval = timeval {
				tv_sec: (proposed / 1_000_000) as time_t,
				tv_usec: (proposed % 1_000_000) as suseconds_t,
			};
			let duration = itimerval {
				it_interval: interval,
				it_value: interval,
			};
			setitimer(Timer::Real, &duration, None).unwrap();

			let mut last = QUANTUM.compare_and_swap(current, proposed, Ordering::Relaxed);
			swap(&mut current, &mut last);
			last != current
		} else {
			return false;
		}
	} {}

	true
}

fn search_update_earliest(call_stack: &[UntypedContinuation]) {
	let time_out = call_stack.iter()
		.map(|frame| frame.time_out).enumerate()
		.map(|(index, time_out)| (time_out, index)).min()
		.map(|(_, index)| index);

	EARLIEST.with(|earliest| earliest.set(time_out));
}

fn nsnow() -> u64 {
	let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
	now.as_secs() * 1_000_000_000 + now.subsec_nanos() as u64
}

#[cfg(test)]
mod tests {
	use signal::tests_sigalrm_lock;
	use super::*;
	#[cfg(bench)]
	use test::Bencher;

	#[test]
	fn launch_completion() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		assert!(launch(|| (), 1_000).unwrap().is_completion());
		drop(lock);
	}

	#[test]
	fn launch_continuation() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		assert!(launch(|| timeout(1_000_000), 10).unwrap().is_continuation());
		drop(lock);
	}

	#[should_panic(expected = "PASS")]
	#[test]
	fn launch_panic() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		drop(launch(|| panic!("PASS"), 1_000));
		// Lock becomes poisoned.
	}

	#[should_panic(expected = "PASS")]
	#[test]
	fn launch_panic_outer() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		drop(launch(|| {
			drop(launch(|| (), 1_000));
			panic!("PASS");
		}, 1_000));
		// Lock becomes poisoned.
	}

	#[should_panic(expected = "PASS")]
	#[test]
	fn launch_panic_inner() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		drop(launch(|| launch(|| panic!("PASS"), 1_000), 1_000));
		// Lock becomes poisoned.
	}

	#[test]
	fn launch_completions() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		assert!(launch(|| assert!(launch(|| (), 1_000).unwrap().is_completion()), 1_000).unwrap().is_completion());
		drop(lock);
	}

	#[test]
	fn launch_continuations() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		assert!(launch(|| {
			assert!(launch(|| timeout(1_000_000), 10).unwrap().is_continuation());
			timeout(1_000_000);
		}, 1_000).unwrap().is_continuation());
		drop(lock);
	}

	#[test]
	fn resume_completion() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		if let Linger::Continuation(cont) = launch(|| timeout(1_000_000), 10).unwrap() {
			assert!(resume(cont, 10_000_000).unwrap().is_completion());
		} else {
			unreachable!("completion instead of continuation!");
		}
		drop(lock);
	}

	#[test]
	fn resume_completion_drop() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		if let Linger::Continuation(cont) = launch(|| timeout(1_000_000), 100).unwrap() {
			assert!(resume(cont, 10_000).unwrap().is_continuation());
		} else {
			unreachable!("completion instead of continuation!");
		}
		drop(lock);
	}

	#[test]
	fn resume_completion_repeat() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		if let Linger::Continuation(cont) = launch(|| timeout(1_000_000), 10).unwrap() {
			if let Linger::Continuation(cont) = resume(cont, 10).unwrap() {
				assert!(resume(cont, 10_000_000).unwrap().is_completion());
			} else {
				unreachable!("inner: completion instead of continuation!");
			}
		} else {
			unreachable!("outer: completion instead of continuation!");
		}
		drop(lock);
	}

	#[test]
	fn abuse_preemption() {
		for _ in 0..10_000 {
			launch_continuation();
		}
	}

	fn timeout(mut useconds: u64) {
		useconds *= 1_000;

		let mut elapsed = 0;
		let mut last = nsnow();
		while elapsed < useconds {
			let mut this = nsnow();
			while this < last || this - last > 1_000 {
				last = this;
				this = nsnow();
			}
			elapsed += this - last;
			last = this;
		}
	}

	#[bench]
	#[cfg(bench)]
	fn timeout_10(lo: &mut Bencher) {
		lo.iter(|| timeout(10));
	}

	#[bench]
	#[cfg(bench)]
	fn timeout_100(lo: &mut Bencher) {
		lo.iter(|| timeout(100));
	}

	#[bench]
	#[cfg(bench)]
	fn timeout_1000(lo: &mut Bencher) {
		lo.iter(|| timeout(1_000));
	}
}
