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
use std::cmp::min;
pub use std::io::Error;
use std::iter::once;
use std::mem::swap;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::panic::resume_unwind;
use std::rc::Rc;
use std::sync::atomic::ATOMIC_USIZE_INIT;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::ONCE_INIT;
use std::sync::Once;
use std::thread::Result;
use std::time::UNIX_EPOCH;
use std::time::SystemTime;
use time::Timer;
use time::itimerval;
use time::setitimer;
use time::timeval;
use ucontext::REG_CSGSFS;
use ucontext::getcontext;
use ucontext::makecontext;
use ucontext::swapcontext;
use volatile::VolBool;
use zeroable::Zeroable;

const TIME_QUANTUM_DIVISOR: u64 = 3;

static QUANTUM: AtomicUsize = ATOMIC_USIZE_INIT;

thread_local! {
	static EARLIEST: Cell<usize> = Cell::new(0);
}

enum LaunchResume<T, F: FnMut() -> T> {
	Launch(F),
	Resume((Option<UntypedContinuation>, Vec<UntypedContinuation>)),
}

impl<'a, T: 'a, F: 'a + FnMut() -> T> LaunchResume<T, F> {
	fn into_fn_mut(self, res: Rc<Cell<Result<T>>>) -> Box<'a + FnMut()> {
		match self {
			LaunchResume::Launch(mut fun) => Box::new(move ||
				res.set(catch_unwind(AssertUnwindSafe(&mut fun)))
			),
			LaunchResume::Resume((_, mut tail)) => Box::new(move ||
				CallStack::handle().unwrap().lock().map(|mut call_stack| {
					let ts = nsnow();
					for frame in &mut tail {
						frame.time_out += ts;
					}
					call_stack.append(&mut tail);
					teardown(&mut call_stack);
				}).unwrap()
			),
		}
	}
}

pub struct Continuation<T, F: FnMut() -> T> {
	function: LaunchResume<T, F>,
	complete: Box<ucontext_t>,
	result: Rc<Cell<Result<T>>>,
}

pub enum Linger<T, F: FnMut() -> T> {
	Completion(T),
	Continuation(Continuation<T, F>),
	Failure(Error),
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

	pub fn is_failure(&self) -> bool {
		if let &Linger::Failure(_) = self {
			true
		} else {
			false
		}
	}
}

pub fn launch<T: 'static, F: 'static + FnMut() -> T>(fun: F, us: u64) -> Linger<T, F> {
	resume(Continuation {
		function: LaunchResume::Launch(fun),
		complete: Box::new(ucontext_t::new()),
		result: Rc::new(Cell::new(Err(Box::new(())))),
	}, us)
}

pub fn resume<T: 'static, F: 'static + FnMut() -> T>(mut funs: Continuation<T, F>, us: u64) -> Linger<T, F> {
	static INIT_HANDLER: Once = ONCE_INIT;
	INIT_HANDLER.call_once(|| {
		let handler = Sigaction::new(preempt, Sigset::empty(), SA_SIGINFO | SA_RESTART);
		sigaction(Signal::Alarm, &handler, None).unwrap();
	});

	let resuming;
	let mut call_gate = ucontext_t::new();
	let mut frame;
	if let &mut LaunchResume::Resume((ref mut head, _)) = &mut funs.function {
		let mut head = head.take().unwrap();

		resuming = true;
		head.time_limit = us;
		swap(&mut call_gate, &mut head.pause_resume.borrow_mut());
		frame = head;
	} else {
		resuming = false;
		frame = UntypedContinuation::new(us);
	}
	frame.thunk = Some(funs.function.into_fn_mut(funs.result.clone()));

	let mut complete = funs.complete;
	let result = funs.result;
	if ! resuming {
		if let Err(or) = makecontext(&mut call_gate, preemptor, &mut frame.stack, Some(&mut complete)) {
			return Linger::Failure(or);
		}
	}

	let pause = frame.pause_resume.clone();
	let index;
	match CallStack::handle().unwrap().lock() {
		Err(or) => return Linger::Failure(or),
		Ok(mut call_stack) => {
			index = call_stack.len();
			call_stack.push(frame);
		},
	}

	let mut timeout = VolBool::new(false);
	if let Err(or) = getcontext(&mut pause.borrow_mut()) {
		return Linger::Failure(or);
	}

	if ! timeout.get() {
		timeout.set(true);

		if resuming {
			preemptor();
		}
		if let Err(or) = swapcontext(&mut complete, &mut call_gate) {
			return Linger::Failure(or);
		}

		CallStack::handle().unwrap().lock()
			.map(|mut call_stack| {
				call_stack.pop();
				teardown(&call_stack);

				let res = Rc::try_unwrap(result).ok().unwrap().into_inner();
				let res = res.unwrap_or_else(|panic| resume_unwind(panic));
				Linger::Completion(res)
			})
			.unwrap_or_else(|err| Linger::Failure(err))
	} else {
		CallStack::handle().unwrap().lock()
			.and_then(|mut call_stack| {
				let ts = nsnow();

				let mut tail = call_stack.split_off(index + 1);
				let mut head = call_stack.pop().unwrap();
				teardown(&call_stack);
				for frame in once(&mut head).chain(&mut tail) {
					frame.time_out -= min(ts, frame.time_out);
				}

				Ok(Linger::Continuation(Continuation {
					function: LaunchResume::Resume((Some(head), tail)),
					complete: complete,
					result: result,
				}))
			})
			.unwrap_or_else(|err| Linger::Failure(err))
	}
}

fn teardown(call_stack: &Vec<UntypedContinuation>) {
	let earliest_time_out = call_stack.iter()
		.map(|frame| frame.time_out).enumerate()
		.min_by_key(|&(_, time_out)| time_out)
		.map(|(index, _)| index).unwrap_or(0);
	EARLIEST.with(|earliest| earliest.set(earliest_time_out));

	let shortest = call_stack.iter().map(|frame| frame.time_limit).min();
	let quantum_time_limit = shortest.unwrap_or(0);

	while {
		let quantum = QUANTUM.load(Ordering::Relaxed);

		if quantum_time_limit < min_nonzero(quantum as u64) {
			let interval = timeval {
				tv_sec: (quantum_time_limit / 1_000_000) as time_t,
				tv_usec: (quantum_time_limit % 1_000_000) as suseconds_t,
			};
			let duration = itimerval {
				it_interval: interval,
				it_value: interval,
			};
			setitimer(Timer::Real, &duration, None).unwrap();

			QUANTUM.compare_and_swap(quantum, quantum_time_limit as usize, Ordering::Relaxed) != quantum
		} else {
			false
		}
	} {}
}

extern "C" fn preemptor() {
	// Take a thread-wide preemption lock.
	let mut thunk = CallStack::handle().unwrap().lock().map(|mut call_stack| {
		let earliest = call_stack.get(EARLIEST.with(|earliest| earliest.get()))
			.map(|frame| frame.time_out)
			.unwrap_or(0);

		let index = call_stack.len() - 1;
		let frame = call_stack.last_mut().unwrap();
		let thunk = frame.thunk.take().unwrap();
		let time_limit = frame.time_limit;

		let my_quantum = time_limit / TIME_QUANTUM_DIVISOR;
		while {
			let quantum = QUANTUM.load(Ordering::Relaxed);

			if my_quantum < min_nonzero(quantum as u64) {
				let interval = timeval {
					tv_sec: (my_quantum / 1_000_000) as time_t,
					tv_usec: (my_quantum % 1_000_000) as suseconds_t,
				};
				let duration = itimerval {
					it_interval: interval,
					it_value: interval,
				};
				setitimer(Timer::Real, &duration, None).unwrap();

				QUANTUM.compare_and_swap(quantum, my_quantum as usize, Ordering::Relaxed) != quantum
			} else {
				false
			}
		} {}

		let time_out = nsnow() + time_limit * 1_000;
		if time_out < min_nonzero(earliest) {
			EARLIEST.with(|earliest| earliest.set(index));
		}
		frame.time_out = time_out;

		thunk

		// Release our lock, enabling preemption!
	}).unwrap();

	thunk();
}

extern "C" fn preempt(signum: Signal, _: Option<&siginfo_t>, sigctxt: Option<&mut ucontext_t>) {
	debug_assert!(signum == Signal::Alarm);

	if let Ok(handle) = CallStack::handle() {
		if let Ok(mut call_stack) = handle.preempt() {
			let index = EARLIEST.with(|earliest| earliest.get());
			if let Some(frame) = call_stack.get_mut(index) {
				if nsnow() < min_nonzero(frame.time_out) {
					return;
				}

				let sigctxt = sigctxt.unwrap();
				let segs = sigctxt.uc_mcontext.gregs[REG_CSGSFS];
				swap(&mut *frame.pause_resume.borrow_mut(), sigctxt);
				sigctxt.uc_mcontext.gregs[REG_CSGSFS] = segs;

				frame.time_out = 0;
			}
		}
	}
}

fn min_nonzero(num: u64) -> u64 {
	if num != 0 {
		num
	} else {
		u64::max_value()
	}
}

fn nsnow() -> u64 {
	let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
	now.as_secs() * 1_000_000_000 + now.subsec_nanos() as u64
}

#[cfg(test)]
mod tests {
	use linger::*;
	use signal::tests_sigalrm_lock;
	use test::Bencher;

	#[test]
	fn launch_completion() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		assert!(launch(|| (), 1_000).is_completion());
		drop(lock);
	}

	#[test]
	fn launch_continuation() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		assert!(launch(|| timeout(1_000_000), 10).is_continuation());
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
		assert!(launch(|| assert!(launch(|| (), 1_000).is_completion()), 1_000).is_completion());
		drop(lock);
	}

	#[test]
	fn launch_continuations() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		assert!(launch(|| {
			assert!(launch(|| timeout(1_000_000), 10).is_continuation());
			timeout(1_000_000);
		}, 1_000).is_continuation());
		drop(lock);
	}

	#[test]
	fn resume_completion() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		if let Linger::Continuation(cont) = launch(|| timeout(1_000_000), 10) {
			assert!(resume(cont, 10_000_000).is_completion());
		} else {
			unreachable!("completion instead of continuation!");
		}
		drop(lock);
	}

	#[test]
	fn resume_completion_drop() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		if let Linger::Continuation(cont) = launch(|| timeout(1_000_000), 100) {
			assert!(resume(cont, 10_000).is_continuation());
		} else {
			unreachable!("completion instead of continuation!");
		}
		drop(lock);
	}

	#[test]
	fn resume_completion_repeat() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		if let Linger::Continuation(cont) = launch(|| timeout(1_000_000), 10) {
			if let Linger::Continuation(cont) = resume(cont, 10) {
				assert!(resume(cont, 10_000_000).is_completion());
			} else {
				unreachable!("inner: completion instead of continuation!");
			}
		} else {
			unreachable!("outer: completion instead of continuation!");
		}
		drop(lock);
	}

	fn timeout(mut useconds: u64) {
		useconds *= 1_000;

		let mut elapsed = 0;
		let mut last = nsnow();
		while elapsed < useconds {
			let mut this = nsnow();
			while this - last > 1_000 {
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
