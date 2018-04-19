use continuation::CallStack;
use continuation::UntypedContinuation;
use libc::SA_RESTART;
use libc::SA_SIGINFO;
use libc::siginfo_t;
use libc::suseconds_t;
use libc::time_t;
use libc::ucontext_t;
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
use ucontext::REG_CSGSFS;
use ucontext::getcontext;
use ucontext::makecontext;
use ucontext::setcontext;
use ucontext::swap;

const TIME_QUANTUM_DIVISOR: u64 = 3;

static QUANTUM: AtomicUsize = ATOMIC_USIZE_INIT;

thread_local! {
	// None means the top of the call_stack.
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
	static INIT_HANDLER: Once = ONCE_INIT;
	INIT_HANDLER.call_once(|| {
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
	if let Some(first_time_here) = getcontext()? {
		let mut call_stack = CallStack::lock()?;
		match funs.function {
			LaunchResume::Launch(mut thunk) => {
				let ult: Rc<Cell<PanicResult<T>>> = Rc::new(Cell::new(Err(Box::new(()))));
				res = Rc::downgrade(&ult);
				let thunk = move || {
					let res = catch_unwind(AssertUnwindSafe(&mut thunk));
					EARLIEST.with(|earliest| earliest.set(None));
					ult.set(res);
				};

				let mut frame = UntypedContinuation::new(thunk, us, first_time_here);
				let mut call_gate = makecontext(preemptor, &mut frame.stack,
					Some(&mut frame.pause_resume))?;
				call_stack.push(frame);

				drop(call_stack);
				setcontext(&mut call_gate)?;
			},
			LaunchResume::Resume((mut cont, mut inuations)) => {
				use ucontext::fixupcontext;

				let mut checkpoint = *cont.pause_resume;
				*cont.pause_resume = first_time_here;
				fixupcontext(&mut cont.pause_resume);
				checkpoint.uc_link = &mut *cont.pause_resume;

				let thunk = cont.thunk;
				let thunk = Box::new(move || {
					let _ = thunk;

					if ! inuations.is_empty() {
						let mut call_stack = CallStack::lock().unwrap();

						let quantum = inuations.iter()
							.map(|frame| frame.time_limit).min()
							.unwrap_or(u64::max_value()) / TIME_QUANTUM_DIVISOR;
						maybe_update_quantum(quantum);

						let ts = nsnow();
						for frame in inuations.iter_mut() {
							frame.time_out += ts;
						}

						call_stack.append(&mut inuations);
						search_update_earliest(&call_stack);
					}

					setcontext(&mut checkpoint).unwrap();
				});
				cont.thunk = thunk;
				cont.time_limit = us;

				call_stack.push(cont);

				drop(call_stack);
				preemptor();
			},
		}
	}

	let mut call_stack = CallStack::lock()?;
	let index = EARLIEST.with(|earliest| earliest.take())
		.map(|earliest| earliest + 1).unwrap_or(call_stack.len());
	let mut tail = call_stack.split_off(index);
	let head = call_stack.pop().unwrap();

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

extern "C" fn preemptor() {
	let mut call_stack = CallStack::lock().unwrap();
	let index = call_stack.len() - 1;

	let thunk;
	let time_out;
	{
		let frame = call_stack.last_mut().unwrap();
		let ptr: *mut _ = &mut *frame.thunk;
		thunk = unsafe {
			ptr.as_mut()
		}.unwrap();

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

	drop(call_stack);
	thunk();
}

extern "C" fn preempt(_: Signal, _: Option<&siginfo_t>, sigctxt: Option<&mut ucontext_t>) {
	if let Ok(mut call_stack) = unsafe {
		CallStack::preempt()
	} {
		if let Some(index) = EARLIEST.with(|earliest| earliest.get()) {
			let frame = &mut call_stack[index];
			if nsnow() < frame.time_out {
				return;
			}
			frame.time_out = 0;
			EARLIEST.with(|earliest| earliest.set(Some(index)));

			let sigctxt = sigctxt.unwrap();
			let segs = sigctxt.uc_mcontext.gregs[REG_CSGSFS];
			swap(sigctxt, &mut *frame.pause_resume);
			sigctxt.uc_mcontext.gregs[REG_CSGSFS] = segs;

			// No more preemptions until resume() has finished bundling up the
			// continuation, at which point they will be automatically reenabled
			sigctxt.uc_sigmask.add(Signal::Alarm);
		}
	}
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
	fn timeout_10(lo: &mut Bencher) {
		lo.iter(|| timeout(10));
	}

	#[bench]
	fn timeout_100(lo: &mut Bencher) {
		lo.iter(|| timeout(100));
	}

	#[bench]
	fn timeout_1000(lo: &mut Bencher) {
		lo.iter(|| timeout(1_000));
	}
}
