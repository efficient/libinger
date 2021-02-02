use gotcha::Group;
use gotcha::group_thread_set;
use reusable::ReusableSync;
use signal::Handler;
use signal::Operation;
use signal::Set;
use signal::Signal;
use signal::Sigset;
use signal::sigaction;
use std::cell::Cell;
use std::cell::RefCell;
use std::io::Result as IoResult;
use timer::Timer;

thread_local! {
	static SIGNAL: RefCell<Option<RealThreadId>> = RefCell::default();

	// Whether we had to delay preemption checks until the end of a nonpreemptible call.
	static DEFERRED: Cell<bool> = Cell::new(false);
}

pub fn thread_signal() -> Result<Signal, ()> {
	// Because this is called from signal handlers, it might happen during thread teardown, when
	// the thread-local variable is being/has been destructed.  In such a case, we simply report
	// that the current thread has no preemption signal assigned (any longer).
	SIGNAL.try_with(|signal|
		signal.borrow().as_ref().map(|signal| {
			let RealThreadId (signal) = signal;
			signal.borrow().as_ref().map(|signal|
				*signal.signal
			)
		}).unwrap_or(None).ok_or(())
	).unwrap_or(Err(()))
}

extern fn resume_preemption() {
	// Skip if this trampoline is running in a destructor during thread teardown.
	drop(enable_preemption(None));
}

pub fn enable_preemption(group: Option<Group>) -> Result<(), ()> {
	use signal::pthread::pthread_kill;
	use signal::pthread::pthread_self;

	// We can only call thread_signal() if the preemption signal is already blocked; otherwise,
	// the signal handler might race on the thread-local SIGNAL variable.  It's fine to do when:
	//  * We have been passed a group, because in this case preemption was previously disabled.
	//    - OR -
	//  * Preemption has been deferred, because when setting the flag, the signal handler will
	//    have masked out the signal.
	let mut unblock = None;
	if let Some(group) = group {
		// It's important we don't unmask the preemption signal until we've switched groups;
		// otherwise, its handler may run immediately and remask it!
		group_thread_set!(group);
		unblock.replace(thread_signal()?);
	}
	// else the caller is asserting the group change has already been performed.

	if DEFERRED.with(|deferred| deferred.replace(false)) {
		let signal = unblock.get_or_insert(thread_signal()?);
		drop(pthread_kill(pthread_self(), *signal));
	}

	if let Some(signal) = unblock {
		drop(mask(Operation::Unblock, signal));
	}

	Ok(())
}

pub fn disable_preemption() {
	group_thread_set!(Group::SHARED);
	SIGNAL.with(|signal| signal.replace(None));
	DEFERRED.with(|deferred| deferred.replace(false));
}

pub fn is_preemptible() -> bool {
	use gotcha::group_thread_get;

	! group_thread_get!().is_shared()
}

pub fn defer_preemption() {
	DEFERRED.with(|deferred| deferred.replace(true));
}

pub fn thread_setup(thread: RealThreadId, handler: Handler, quantum: u64) -> IoResult<()> {
	use gotcha::shared_hook;
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	let RealThreadId (signal) = thread;
	if signal.borrow().is_none() {
		signal.replace(Some(PreemptionSignal::new(handler, quantum)?));
	}
	SIGNAL.with(|signal| signal.replace(Some(thread)));

	static INIT: Once = ONCE_INIT;
	INIT.call_once(|| shared_hook(resume_preemption));

	Ok(())
}

fn mask(un: Operation, no: Signal) -> IoResult<()> {
	use signal::pthread_sigmask;

	let mut set = Sigset::empty();
	set.add(no);
	pthread_sigmask(un, &set, None)
}

pub struct RealThreadId (&'static RefCell<Option<PreemptionSignal>>);

impl RealThreadId {
	pub fn current() -> Self {
		use lifetime::unbound;

		thread_local! {
			static SIGNALER: RefCell<Option<PreemptionSignal>> = RefCell::default();
		}

		Self (SIGNALER.with(|signaler| unsafe {
			unbound(signaler)
		}))
	}
}

struct PreemptionSignal {
	signal: ReusableSync<'static, Signal>,
	timer: Timer,
}

impl PreemptionSignal {
	fn new(handler: Handler, quantum: u64) -> IoResult<Self> {
		use libc::SA_RESTART;
		use libc::SA_SIGINFO;
		use libc::timespec;
		use signal::Action;
		use signal::Sigaction;
		use signals::assign_signal;
		use timer::Clock;
		use timer::Sigevent;
		use timer::itimerspec;
		use timer::timer_create;
		use timer::timer_settime;

		let signal = assign_signal().expect("libinger: no available signal for preempting this thread");
		let sa = Sigaction::new(handler, Sigset::empty(), SA_SIGINFO | SA_RESTART);
		sigaction(*signal, &sa, None)?;
		mask(Operation::Block, *signal)?;

		let mut se = Sigevent::signal(*signal);
		let timer = timer_create(Clock::Real, &mut se)?;
		let quantum: i64 = quantum as _;
		let mut it = itimerspec {
			it_interval: timespec {
				tv_sec: 0,
				tv_nsec: quantum * 1_000,
			},
			it_value: timespec {
				tv_sec: 0,
				tv_nsec: quantum * 1_000,
			},
		};
		timer_settime(timer, false, &mut it, None)?;

		Ok(Self {
			signal,
			timer,
		})
	}
}

impl Drop for PreemptionSignal {
	fn drop(&mut self) {
		use timer::timer_delete;

		if let Err(or) = timer_delete(self.timer) {
			eprintln!("libinger: unable to delete POSIX timer: {}", or);
		}
		if let Err(or) = sigaction(*self.signal, &(), None) {
			eprintln!("libinger: unable to unregister signal handler: {}", or);
		}
	}
}

