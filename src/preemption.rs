use gotcha::Group;
use gotcha::group_thread_set;
use reusable::ReusableSync;
use signal::Handler;
use signal::Operation;
use signal::Set;
use signal::Signal;
use signal::Sigset;
use signals::assign_signal;
use std::cell::Cell;
use std::io::Result;

thread_local! {
	static SIGNAL: ReusableSync<'static, Signal> =
		assign_signal().expect("libinger: no available signal for preempting this thread");

	// Whether we had to delay preemption checks until the end of a nonpreemptible call.
	static DEFERRED: Cell<bool> = Cell::new(false);
}

pub fn thread_signal() -> Signal {
	SIGNAL.with(|signal| **signal)
}

extern fn resume_preemption() {
	enable_preemption(None);
}

pub fn enable_preemption(group: Option<Group>) {
	use signal::pthread::pthread_kill;
	use signal::pthread::pthread_self;

	let mut unblock = false;
	if let Some(group) = group {
		// It's important we don't unmask the preemption signal until we've switched groups;
		// otherwise, its handler may run immediately and remask it!
		group_thread_set!(group);
		unblock = true;
	}
	// else the caller is asserting the group change has already been performed.

	let signal = thread_signal();
	DEFERRED.with(|deferred| if deferred.replace(false) {
		drop(pthread_kill(pthread_self(), signal));
		unblock = true;
	});

	if unblock {
		drop(mask(Operation::Unblock, signal));
	}
}

pub fn disable_preemption() {
	group_thread_set!(Group::SHARED);
	DEFERRED.with(|deferred| deferred.replace(false));
}

pub fn is_preemptible() -> bool {
	use gotcha::group_thread_get;

	! group_thread_get!().is_shared()
}

pub fn defer_preemption() {
	DEFERRED.with(|deferred| deferred.replace(true));
}

pub fn thread_setup(handler: Handler, quantum: u64) -> Result<()> {
	use gotcha::shared_hook;
	use libc::SA_RESTART;
	use libc::SA_SIGINFO;
	use libc::timespec;
	use signal::Action;
	use signal::Sigaction;
	use signal::sigaction;
	use std::sync::ONCE_INIT;
	use std::sync::Once;
	use timer::Clock;
	use timer::Sigevent;
	use timer::Timer;
	use timer::itimerspec;
	use timer::timer_create;
	use timer::timer_settime;

	thread_local! {
		static TIMER: Cell<Option<Timer>> = Cell::default();
	}
	if TIMER.with(|timer| timer.get()).is_none() {
		let signal = thread_signal();
		let sa = Sigaction::new(handler, Sigset::empty(), SA_SIGINFO | SA_RESTART);
		sigaction(signal, &sa, None)?;
		mask(Operation::Block, signal)?;

		let mut se = Sigevent::signal(signal);
		let alarm = timer_create(Clock::Real, &mut se)?;
		TIMER.with(|timer| timer.replace(alarm.into()));

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
		timer_settime(alarm, false, &mut it, None)?;
	}

	static INIT: Once = ONCE_INIT;
	INIT.call_once(|| shared_hook(resume_preemption));

	Ok(())
}

fn mask(un: Operation, no: Signal) -> Result<()> {
	use signal::pthread_sigmask;

	let mut set = Sigset::empty();
	set.add(no);
	pthread_sigmask(un, &set, None)
}
