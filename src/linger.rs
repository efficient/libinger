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
use std::mem::uninitialized;
use std::rc::Rc;
use std::sync::ONCE_INIT;
use std::sync::Once;
use time::Timer;
use time::setitimer;
use ucontext::REG_CSGSFS;
use ucontext::getcontext;
use ucontext::makecontext;
use ucontext::swapcontext;
use zeroable::Zeroable;

const STACK_SIZE_BYTES: usize = 2 * 1_024 * 1_024;

#[must_use = "Lingerless contexts leak if neither destroy()'d nor allowed to resume() running to completion"]
pub struct Continuation<T> (Box<UntypedContinuation>, PhantomData<T>);

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

#[allow(unused_assignments)]
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

	let res: Rc<Cell<T>> = Rc::new(Cell::new(unsafe {
		uninitialized()
	}));
	let frame = {
		let res = res.clone();
		move || res.set(fun())
	};
	let frame = Box::new(UntypedContinuation::new(frame, us));

	let (pause, complete, stack) = CALL_STACK.with(|call_stack| {
		call_stack.borrow_mut().push(frame);
		let frame = call_stack.borrow();
		let frame = frame.last().unwrap();
		(frame.pause.clone(), frame.complete.clone(), frame.stack.clone())
	});

	let mut call_gate = ucontext_t::new();
	if let Err(or) = makecontext(&mut call_gate, preemptor, &mut stack.borrow_mut(), Some(&mut complete.borrow_mut())) {
		return Linger::Failure(or);
	}
	drop(stack);

	let mut timeout = false;
	if let Err(or) = getcontext(&mut pause.borrow_mut()) {
		return Linger::Failure(or);
	}
	drop(pause);

	if ! timeout {
		timeout = true;

		if let Err(or) = swapcontext(&mut complete.borrow_mut(), &mut call_gate) {
			return Linger::Failure(or);
		}
		CALL_STACK.with(|call_stack| call_stack.borrow_mut().pop());

		Linger::Completion(Rc::try_unwrap(res).ok().unwrap().into_inner())
	} else {
		drop(complete);

		if let Err(or) = sigprocmask(Operation::Block, &mask, None) {
			return Linger::Failure(or);
		}

		if CALL_STACK.with(|call_stack| call_stack.borrow_mut().is_empty()) {
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

		Linger::Continuation(Continuation (CALL_STACK.with(|call_stack| call_stack.borrow_mut().pop()).unwrap(), PhantomData::default()))
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
	use std::thread::sleep;
	use std::time::Duration;

	#[test]
	fn launch_completion() {
		use signal::tests_sigalrm_lock;

		let lock = tests_sigalrm_lock();
		assert!(launch(|| sleep(Duration::new(0, 6_000)), 1_000).is_completion());
		drop(lock);
	}
}
