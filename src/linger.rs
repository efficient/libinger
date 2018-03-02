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
use std::cell::RefCell;
pub use std::io::Error;
use std::marker::PhantomData;
use std::mem::swap;
use time::Timer;
use time::setitimer;
use ucontext::REG_CSGSFS;
use ucontext::getcontext;
use zeroable::Zeroable;

#[must_use = "Lingerless contexts leak if neither destroy()'d nor allowed to resume() running to completion"]
pub struct Continuation<T> (ucontext_t, PhantomData<T>);

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
	static ENTRY_POINT: RefCell<ucontext_t> = RefCell::new(ucontext_t::new());
}

#[allow(unused_assignments)]
pub fn launch<T, F: FnOnce() -> T>(fun: F, us: u64) -> Linger<T> {
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

	let handler = Sigaction::new(preempt, Sigset::empty(), SA_SIGINFO | SA_RESTART);
	if let Err(or) = sigaction(Signal::Alarm, &handler, None) {
		return Linger::Failure(or);
	}

	let mut mask = Sigset::empty();
	mask.add(Signal::Alarm);
	if let Err(or) = sigprocmask(Operation::Unblock, &mask, None) {
		return Linger::Failure(or);
	}

	if let Err(or) = ENTRY_POINT.with(|entry_point| getcontext(&mut entry_point.borrow_mut())) {
		return Linger::Failure(or);
	}

	let mut timeout = false;
	if ! timeout {
		timeout = true;

		let duration = itimerval {
			it_interval: timeval {
				tv_sec: 0,
				tv_usec: 0,
			},
			it_value: timeval {
				tv_sec: (us / 1_000_000) as time_t,
				tv_usec: (us % 1_000_000) as suseconds_t,
			},
		};
		if let Err(or) = setitimer(Timer::Real, &duration, None) {
			return Linger::Failure(or);
		}

		let res = fun();
		if let Err(or) = setitimer(Timer::Real, &NEVER, None) {
			return Linger::Failure(or);
		}

		Linger::Completion(res)
	} else {
		Linger::Continuation(Continuation(ENTRY_POINT.with(|entry_point| *entry_point.borrow()), PhantomData::default()))
	}
}

pub fn resume<T>(_: Continuation<T>, _: u64) -> Linger<T> {
	unimplemented!()
}

pub fn destroy<T>(_: Continuation<T>) {
	unimplemented!();
}

extern "C" fn preempt(signum: Signal, _: Option<&siginfo_t>, sigctxt: Option<&mut ucontext_t>) {
	debug_assert!(signum == Signal::Alarm);

	let sigctxt = sigctxt.unwrap();
	ENTRY_POINT.with(|entry_point| {
		let mut entry_point = entry_point.borrow_mut();
		swap(sigctxt, &mut entry_point);
		sigctxt.uc_mcontext.gregs[REG_CSGSFS] = entry_point.uc_mcontext.gregs[REG_CSGSFS];
	});
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
