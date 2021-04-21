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
use libc::ucontext_t;
use std::io::Error;
use std::ptr::null_mut;
pub use libc::sigaction as Sigaction;
pub use libc::siginfo_t;
pub use libc::sigset_t as Sigset;
use std::io::Result;
use std::os::raw::c_int;

pub type Handler = extern "C" fn(Signal, Option<&siginfo_t>, Option<&mut ucontext_t>);

#[allow(dead_code)]
#[repr(i32)]
pub enum Operation {
	Block = SIG_BLOCK,
	Unblock = SIG_UNBLOCK,
	SetMask = SIG_SETMASK,
}

#[allow(dead_code)]
#[derive(Clone)]
#[derive(Copy)]
#[repr(i32)]
pub enum Signal {
	Abort = SIGABRT,
	Alarm = SIGALRM,
	Bus = SIGBUS,
	Breakpoint = SIGTRAP,
	Child = SIGCHLD,
	Continue = SIGCONT,
	Coprocessor = SIGSTKFLT,
	FilesystemLimit = SIGXFSZ,
	FloatingPoint = SIGFPE,
	Hangup = SIGHUP,
	Illegal = SIGILL,
	Interrupt = SIGINT,
	Kill = SIGKILL,
	Pipe = SIGPIPE,
	Pollable = SIGPOLL,
	ProfilingTimer = SIGPROF,
	Quit = SIGQUIT,
	Segfault = SIGSEGV,
	Syscall = SIGSYS,
	TerminalInput = SIGTTIN,
	TerminalOutput = SIGTTOU,
	TerminalStop = SIGTSTP,
	Terminate = SIGTERM,
	PowerFailure = SIGPWR,
	ProcessorLimit = SIGXCPU,
	UrgentSocket = SIGURG,
	User1 = SIGUSR1,
	User2 = SIGUSR2,
	VirtualAlarm = SIGVTALRM,
	WindowResize = SIGWINCH,
}

impl PartialEq for Signal {
	fn eq(&self, other: &Self) -> bool {
		*self as c_int == *other as c_int
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
			sigaddset(self, signal as _);
		}
	}

	fn del(&mut self, signal: Signal) {
		use libc::sigdelset;
		unsafe {
			sigdelset(self, signal as _);
		}
	}

	fn has(&self, signal: Signal) -> bool {
		use libc::sigismember;
		unsafe {
			sigismember(self, signal as _) != 0
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
		Self {
			sa_sigaction: sigaction as _,
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
