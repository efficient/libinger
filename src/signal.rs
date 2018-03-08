use libc::SIG_BLOCK;
use libc::SIG_SETMASK;
use libc::SIG_UNBLOCK;
use libc::SIGALRM;
use libc::SIGHUP;
use libc::SIGINT;
use libc::SIGPIPE;
use libc::SIGSYS;
use libc::SIGTERM;
use libc::SIGUSR1;
use libc::SIGUSR2;
use libc::c_int;
use libc::ucontext_t;
use std::io::Error;
use std::ptr::null_mut;
pub use libc::sigaction as Sigaction;
use libc::siginfo_t;
pub use libc::sigset_t as Sigset;
use std::io::Result;

pub type Handler = extern "C" fn(Signal, Option<&siginfo_t>, Option<&mut ucontext_t>);

#[allow(dead_code)]
pub enum Operation {
	Block = SIG_BLOCK as isize,
	Unblock = SIG_UNBLOCK as isize,
	SetMask = SIG_SETMASK as isize,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum Signal {
	Alarm = SIGALRM as isize,
	Hangup = SIGHUP as isize,
	Interrupt = SIGINT as isize,
	Pipe = SIGPIPE as isize,
	Syscall = SIGSYS as isize,
	Term = SIGTERM as isize,
	User1 = SIGUSR1 as isize,
	User2 = SIGUSR2 as isize,
}

impl PartialEq for Signal {
	fn eq(&self, other: &Self) -> bool {
		self.clone() as isize == other.clone() as isize
	}
}
impl Eq for Signal {}

pub trait Set {
	fn empty() -> Self;
	fn full() -> Self;
	fn add(&mut self, Signal);
	fn del(&mut self, Signal);
	fn has(&self, Signal) -> bool;
}

fn sigset(fun: fn(&mut Sigset)) -> Sigset {
	use std::mem::zeroed;

	let mut my = unsafe {
		zeroed()
	};
	fun(&mut my);
	my
}

impl Set for Sigset {
	fn empty() -> Self {
		use libc::sigemptyset;
		sigset(|me| unsafe { sigemptyset(me); })
	}

	fn full() -> Self {
		use libc::sigfillset;
		sigset(|me| unsafe { sigfillset(me); })
	}

	fn add(&mut self, signal: Signal) {
		use libc::sigaddset;
		unsafe {
			sigaddset(self, signal as c_int);
		}
	}

	fn del(&mut self, signal: Signal) {
		use libc::sigdelset;
		unsafe {
			sigdelset(self, signal as c_int);
		}
	}

	fn has(&self, signal: Signal) -> bool {
		use libc::sigismember;
		unsafe {
			sigismember(self, signal as c_int) != 0
		}
	}
}

pub trait Action {
	fn new(Handler, Sigset, c_int) -> Self;
	fn sa_sigaction(&self) -> &Handler;
	fn sa_sigaction_mut(&mut self) -> &mut Handler;
}

impl Action for Sigaction {
	fn new(sigaction: Handler, mask: Sigset, flags: c_int) -> Self {
		use libc::size_t;

		Self {
			sa_sigaction: sigaction as size_t,
			sa_mask: mask,
			sa_flags: flags,
			sa_restorer: None,
		}
	}

	fn sa_sigaction(&self) -> &Handler {
		use std::mem::transmute;

		unsafe {
			transmute(self.sa_sigaction)
		}
	}

	fn sa_sigaction_mut(&mut self) -> &mut Handler {
		use std::mem::transmute;

		unsafe {
			transmute(self.sa_sigaction)
		}
	}
}

pub trait Actionable {
	fn maybe(&self) -> Option<&Sigaction>;
}

impl Actionable for Sigaction {
	fn maybe(&self) -> Option<&Self> {
		Some(self)
	}
}

impl Actionable for () {
	fn maybe(&self) -> Option<&Sigaction> {
		None
	}
}

pub fn sigaction(signal: Signal, new: &Actionable, old: Option<&mut Sigaction>) -> Result<()> {
	use libc::sigaction;

	if unsafe {
		sigaction(
			signal as c_int,
			if let Some(new) = new.maybe() { new } else { null_mut() },
			if let Some(old) = old { old } else { null_mut() },
		)
	} == 0 {
		Ok(())
	} else {
		Err(Error::last_os_error())
	}
}

pub fn sigprocmask(how: Operation, new: &Sigset, old: Option<&mut Sigset>) -> Result<()> {
	pthread_sigmask(how, new, old)
}

pub fn pthread_sigmask(how: Operation, new: &Sigset, old: Option<&mut Sigset>) -> Result<()> {
	use libc::pthread_sigmask;

	let code = unsafe {
		pthread_sigmask(how as c_int, new, if let Some(old) = old { old } else { null_mut() })
	};
	if code == 0 {
		Ok(())
	} else {
		Err(Error::from_raw_os_error(code))
	}
}

#[cfg(test)]
mod tests {
	use signal::*;
	use std::ops::Deref;
	use std::sync::MutexGuard;

	#[test]
	fn sigaction_usr1() {
		use libc::pthread_kill;
		use libc::pthread_self;
		use std::sync::atomic::AtomicBool;
		use std::sync::atomic::Ordering;

		thread_local! {
			static RAN: AtomicBool = AtomicBool::new(false);
		}

		extern "C" fn handler(signum: Signal, _: Option<&siginfo_t>, _: Option<&mut ucontext_t>) {
			RAN.with(|ran| ran.store(signum == Signal::User1, Ordering::Relaxed));
		}

		let conf = Sigaction::new(handler, Sigset::empty(), 0);
		sigaction(Signal::User1, &conf, None).unwrap();

		assert_eq!(0, unsafe {
			use libc::SIGUSR1;
			pthread_kill(pthread_self(), SIGUSR1)
		});

		assert!(RAN.with(|ran| ran.load(Ordering::Relaxed)));
	}

	#[test]
	fn sigprocmask_usr2() {
		use libc::pthread_kill;
		use libc::pthread_self;
		use libc::sigsuspend;
		use std::sync::atomic::AtomicBool;
		use std::sync::atomic::Ordering;

		thread_local! {
			static RAN: AtomicBool = AtomicBool::new(false);
		}

		extern "C" fn handler(signum: Signal, _: Option<&siginfo_t>, _: Option<&mut ucontext_t>) {
			RAN.with(|ran| ran.store(signum == Signal::User2, Ordering::Relaxed));
		}

		let mut mask = Sigset::empty();
		mask.add(Signal::User2);
		sigprocmask(Operation::Block, &mask, None).unwrap();

		let conf = Sigaction::new(handler, Sigset::empty(), 0);
		sigaction(Signal::User2, &conf, None).unwrap();

		assert_eq!(0, unsafe {
			use libc::SIGUSR2;
			pthread_kill(pthread_self(), SIGUSR2)
		});

		assert!(!RAN.with(|ran| ran.load(Ordering::Relaxed)));

		let mut mask = Sigset::full();
		mask.del(Signal::User2);
		unsafe {
			sigsuspend(&mask);
		}

		assert!( RAN.with(|ran| ran.load(Ordering::Relaxed)));
	}

	pub struct Restorer<T, A, B: Deref<Target = Fn(&T)>, F: Deref<Target = (A, B)>> {
		done: T,
		fun: F,
		run: bool,
	}

	impl<T, A: Deref<Target = Fn() -> T>, B: Deref<Target = Fn(&T)>, F: Deref<Target = (A, B)>> Restorer<T, A, B, F> {
		fn new(funs: F) -> Self {
			Self {
				done: funs.0(),
				fun: funs,
				run: true,
			}
		}

		pub fn preserve(&mut self) {
			self.run = false;
		}
	}

	impl<T, A, B: Deref<Target = Fn(&T)>, F: Deref<Target = (A, B)>> Drop for Restorer<T, A, B, F> {
		fn drop(&mut self) {
			if self.run {
				(self.fun.1)(&self.done);
			}
		}
	}

	pub fn sigalrm_lock() -> Restorer<Sigaction, Box<Fn() -> Sigaction>, Box<Fn(&Sigaction)>, MutexGuard<'static, (Box<Fn() -> Sigaction>, Box<Fn(&Sigaction)>)>> {
		use std::sync::ONCE_INIT;
		use std::sync::Once;
		use std::sync::Mutex;

		static INIT: Once = ONCE_INIT;
		static mut LOCK: Option<Mutex<(Box<Fn() -> Sigaction>, Box<Fn(&Sigaction)>)>> = None;

		INIT.call_once(|| {
			let save = || {
				extern "C" fn dummy(_: Signal, _: Option<&siginfo_t>, _: Option<&mut ucontext_t>) {}
				let mut res = Sigaction::new(dummy, Sigset::empty(), 0);
				sigaction(Signal::Alarm, &(), Some(&mut res)).unwrap();
				res
			};
			let restore = |it: &Sigaction| sigaction(Signal::Alarm, it, None).unwrap();

			unsafe {
				LOCK = Some(Mutex::new((Box::new(save), Box::new(restore))))
			}
		});

		// The lock might be poisened because a previous test failed. This is safe to ignore
		// because we should no longer have a race (since the other test's thread is now
		// dead) and we don't need to fail the current test as well.
		Restorer::new(unsafe {
			LOCK.as_ref().unwrap()
		}.lock().unwrap_or_else(|poison| poison.into_inner()))
	}
}

#[cfg(test)]
pub use self::tests::sigalrm_lock as tests_sigalrm_lock;
