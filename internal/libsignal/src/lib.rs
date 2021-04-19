mod libgotcha;
pub mod pthread;

use libc::SIG_BLOCK;
use libc::SIG_SETMASK;
use libc::SIG_UNBLOCK;
use libc::SIGABRT;
use libc::SIGALRM;
use libc::SIGBUS;
use libc::SIGCHLD;
use libc::SIGCONT;
use libc::SIGFPE;
use libc::SIGHUP;
use libc::SIGILL;
use libc::SIGINT;
use libc::SIGKILL;
use libc::SIGPIPE;
use libc::SIGPROF;
use libc::SIGPOLL;
use libc::SIGPWR;
use libc::SIGQUIT;
use libc::SIGSEGV;
use libc::SIGSTKFLT;
use libc::SIGSYS;
use libc::SIGTERM;
use libc::SIGTRAP;
use libc::SIGTSTP;
use libc::SIGTTIN;
use libc::SIGTTOU;
use libc::SIGURG;
use libc::SIGUSR1;
use libc::SIGUSR2;
use libc::SIGVTALRM;
use libc::SIGWINCH;
use libc::SIGXCPU;
use libc::SIGXFSZ;
use libc::c_int;
use libc::ucontext_t;
use std::io::Error;
use std::ptr::null_mut;
pub use libc::sigaction as Sigaction;
pub use libc::siginfo_t;
pub use libc::sigset_t as Sigset;
use std::io::Result;

pub type Handler = extern "C" fn(Signal, Option<&siginfo_t>, Option<&mut ucontext_t>);

#[allow(dead_code)]
pub enum Operation {
	Block = SIG_BLOCK as _,
	Unblock = SIG_UNBLOCK as _,
	SetMask = SIG_SETMASK as _,
}

#[allow(dead_code)]
#[derive(Clone)]
#[derive(Copy)]
pub enum Signal {
	Abort = SIGABRT as _,
	Alarm = SIGALRM as _,
	Bus = SIGBUS as _,
	Breakpoint = SIGTRAP as _,
	Child = SIGCHLD as _,
	Continue = SIGCONT as _,
	Coprocessor = SIGSTKFLT as _,
	FilesystemLimit = SIGXFSZ as _,
	FloatingPoint = SIGFPE as _,
	Hangup = SIGHUP as _,
	Illegal = SIGILL as _,
	Interrupt = SIGINT as _,
	Kill = SIGKILL as _,
	Pipe = SIGPIPE as _,
	Pollable = SIGPOLL as _,
	ProfilingTimer = SIGPROF as _,
	Quit = SIGQUIT as _,
	Segfault = SIGSEGV as _,
	Syscall = SIGSYS as _,
	TerminalInput = SIGTTIN as _,
	TerminalOutput = SIGTTOU as _,
	TerminalStop = SIGTSTP as _,
	Terminate = SIGTERM as _,
	PowerFailure = SIGPWR as _,
	ProcessorLimit = SIGXCPU as _,
	UrgentSocket = SIGURG as _,
	User1 = SIGUSR1 as _,
	User2 = SIGUSR2 as _,
	VirtualAlarm = SIGVTALRM as _,
	WindowResize = SIGWINCH as _,
}

impl PartialEq for Signal {
	fn eq(&self, other: &Self) -> bool {
		*self as isize == *other as isize
	}
}
impl Eq for Signal {}

pub trait Set {
	fn empty() -> Self;
	fn full() -> Self;
	fn add(&mut self, _: Signal);
	fn del(&mut self, _: Signal);
	fn has(&self, _: Signal) -> bool;
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
	fn new(_: Handler, _: Sigset, _: c_int) -> Self;
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

pub fn sigaction(signal: Signal, new: &dyn Actionable, old: Option<&mut Sigaction>) -> Result<()> {
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
	use super::*;

	use pthread::pthread_kill;
	use pthread::pthread_self;

	#[test]
	fn sigaction_usr1() {
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

		pthread_kill(pthread_self(), Signal::User1).unwrap();

		assert!(RAN.with(|ran| ran.load(Ordering::Relaxed)));
	}

	#[test]
	fn sigprocmask_usr2() {
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

		pthread_kill(pthread_self(), Signal::User2).unwrap();

		assert!(!RAN.with(|ran| ran.load(Ordering::Relaxed)));

		let mut mask = Sigset::full();
		mask.del(Signal::User2);
		unsafe {
			sigsuspend(&mask);
		}

		assert!( RAN.with(|ran| ran.load(Ordering::Relaxed)));
	}
}
