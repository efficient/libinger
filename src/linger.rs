use libc::SA_RESTART;
use libc::SA_SIGINFO;
use libc::itimerval;
use libc::siginfo_t;
use libc::suseconds_t;
use libc::time_t;
use libc::timeval;
use libc::ucontext_t;
use signal::Action;
use signal::Operation;
use signal::Set;
use signal::Sigaction;
use signal::Signal;
use signal::Sigset;
use signal::sigaction;
use signal::sigprocmask;
use std::cell::Cell;
use std::cell::RefCell;
pub use std::io::Error;
use std::marker::PhantomData;
use std::mem::swap;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::panic::resume_unwind;
use std::rc::Rc;
use std::sync::ONCE_INIT;
use std::sync::Once;
use std::thread::Result;
use time::Timer;
use time::setitimer;
use ucontext::REG_CSGSFS;
use ucontext::getcontext;
use ucontext::makecontext;
use ucontext::swapcontext;
use volatile::VolBool;
use zeroable::Zeroable;

const STACK_SIZE_BYTES: usize = 2 * 1_024 * 1_024;

#[must_use = "Lingerless contexts leak if neither destroy()'d nor allowed to resume() running to completion"]
pub struct Continuation<T> (Vec<Box<UntypedContinuation>>, PhantomData<T>);

struct UntypedContinuation {
	thunk: Cell<Option<Box<FnMut()>>>,
	timeout: u64,
	stack: Rc<RefCell<Box<[u8]>>>,
	pause: Rc<RefCell<ucontext_t>>,
	complete: Rc<RefCell<ucontext_t>>,
}

impl UntypedContinuation {
	fn new<T: 'static + FnMut()>(thunk: T, timeout: u64) -> Self {
		Self {
			thunk: Cell::new(Some(Box::new(thunk))),
			timeout: timeout,
			stack: Rc::new(RefCell::new(vec![0; STACK_SIZE_BYTES].into_boxed_slice())),
			pause: Rc::new(RefCell::new(ucontext_t::new())),
			complete: Rc::new(RefCell::new(ucontext_t::new())),
		}
	}
}

#[must_use = "Lingerless function results can leak if not checked for continuations"]
pub enum Linger<T> {
	Completion(T),
	Continuation(Continuation<T>),
	Failure(Error),
}

impl<T> Linger<T> {
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

thread_local! {
	static CALL_STACK: RefCell<Vec<Box<UntypedContinuation>>> = RefCell::new(vec![]);
}

pub fn launch<T: 'static, F: 'static + FnMut() -> T>(mut fun: F, us: u64) -> Linger<T> {
	let mut mask = Sigset::empty();
	mask.add(Signal::Alarm);
	let mask = mask;
	if let Err(or) = sigprocmask(Operation::Block, &mask, None) {
		return Linger::Failure(or);
	}

	static INIT_HANDLER: Once = ONCE_INIT;
	INIT_HANDLER.call_once(|| {
		let handler = Sigaction::new(preempt, Sigset::empty(), SA_SIGINFO | SA_RESTART);
		sigaction(Signal::Alarm, &handler, None).unwrap();
	});

	let res: Rc<Cell<Result<T>>> = Rc::new(Cell::new(Err(Box::new(()))));
	let frame = {
		let res = res.clone();

		// It's safe to "promise" unwind-safety because client code is wholly responsible
		// for the (in)consistency of any shared memory data structures writeable by a
		// preempted thunk.
		move || res.set(catch_unwind(AssertUnwindSafe (&mut fun)))
	};
	let frame = Box::new(UntypedContinuation::new(frame, us));

	let (pause, complete, stack, index) = CALL_STACK.with(|call_stack| {
		let mut call_stack = call_stack.borrow_mut();
		let index = call_stack.len();
		call_stack.push(frame);
		let frame = call_stack.last().unwrap();
		(frame.pause.clone(), frame.complete.clone(), frame.stack.clone(), index)
	});

	let mut call_gate = ucontext_t::new();
	if let Err(or) = makecontext(&mut call_gate, preemptor, &mut stack.borrow_mut(), Some(&mut complete.borrow_mut())) {
		return Linger::Failure(or);
	}

	let mut timeout = VolBool::new(false);
	if let Err(or) = getcontext(&mut pause.borrow_mut()) {
		return Linger::Failure(or);
	}

	if ! timeout.get() {
		timeout.set(true);

		if let Err(or) = swapcontext(&mut complete.borrow_mut(), &mut call_gate) {
			return Linger::Failure(or);
		}
		CALL_STACK.with(|call_stack| call_stack.borrow_mut().pop());

		// We must keep these live until we're done working with contexts; otherwise, we
		// might double-decrement the reference counter and free them prematurely, leaving
		// ourselves with dangling pointers!
		drop((stack, pause, complete));

		Linger::Completion(Rc::try_unwrap(res).ok().unwrap().into_inner().unwrap_or_else(|panic| resume_unwind(panic)))
	} else {
		if let Err(or) = sigprocmask(Operation::Block, &mask, None) {
			return Linger::Failure(or);
		}

		let (substack, empty) = CALL_STACK.with(|call_stack| {
			let mut call_stack = call_stack.borrow_mut();
			(call_stack.split_off(index), call_stack.is_empty())
		});

		if empty {
			const NEVER: itimerval = itimerval {
				it_interval: timeval {
					tv_sec: 0,
					tv_usec: 0,
				},
				it_value: timeval {
					tv_sec: 0,
					tv_usec: 0,
				},
			};
			if let Err(or) = setitimer(Timer::Real, &NEVER, None) {
				return Linger::Failure(or);
			}
		}

		// We must keep these live until we're done working with contexts; otherwise, we
		// might double-decrement the reference counter and free them prematurely, leaving
		// ourselves with dangling pointers!
		drop((stack, pause, complete));

		Linger::Continuation(Continuation (substack, PhantomData::default()))
	}
}

pub fn resume<T>(_: Continuation<T>, _: u64) -> Linger<T> {
	unimplemented!()
}

pub fn destroy<T>(_: Continuation<T>) {
	unimplemented!();
}

extern "C" fn preemptor() {
	let (mut thunk, timeout) = CALL_STACK.with(|call_stack| {
		let frame = call_stack.borrow();
		let frame = frame.last().unwrap();
		(frame.thunk.take().unwrap(), frame.timeout)
	});

	let mut mask = Sigset::empty();
	mask.add(Signal::Alarm);
	let mask = mask;
	sigprocmask(Operation::Unblock, &mask, None).unwrap();

	let duration = itimerval {
		it_interval: timeval {
			tv_sec: 0,
			tv_usec: 0,
		},
		it_value: timeval {
			tv_sec: (timeout / 1_000_000) as time_t,
			tv_usec: (timeout % 1_000_000) as suseconds_t,
		},
	};
	setitimer(Timer::Real, &duration, None).unwrap();

	thunk();

	sigprocmask(Operation::Block, &mask, None).unwrap();
}

extern "C" fn preempt(signum: Signal, _: Option<&siginfo_t>, sigctxt: Option<&mut ucontext_t>) {
	debug_assert!(signum == Signal::Alarm);

	let sigctxt = sigctxt.unwrap();

	let segs = sigctxt.uc_mcontext.gregs[REG_CSGSFS];
	CALL_STACK.with(|call_stack| {
		let frame = call_stack.borrow_mut();
		let frame = frame.last().unwrap();
		swap(&mut *frame.pause.borrow_mut(), sigctxt);
	});
	sigctxt.uc_mcontext.gregs[REG_CSGSFS] = segs;
}

#[cfg(test)]
mod tests {
	use linger::*;
	use signal::tests_sigalrm_lock;

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

	#[should_panic]
	#[test]
	fn launch_panic() {
		let mut lock = tests_sigalrm_lock();
		lock.preserve();
		drop(launch(|| panic!(), 1_000));
		// Lock becomes poisoned.
	}

	fn timeout(useconds: u64) {
		use std::thread::sleep;
		use std::time::Duration;

		sleep(Duration::new(useconds / 1_000_000, (useconds % 1_000_000) as u32 * 1_000));
	}
}
