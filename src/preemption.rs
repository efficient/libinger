use gotcha::Group;
use gotcha::group_thread_set;
use reusable::ReusableSync;
use signal::pthread::pthread_kill;
use signal::pthread::pthread_self;
use signal::Handler;
use signal::Operation;
use signal::Set;
use signal::Signal;
use signal::Sigset;
use signal::sigaction;
use std::cell::RefCell;
use std::io::Result as IoResult;
use std::os::raw::c_int;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use timer::Timer;

thread_local! {
	static SIGNAL: RefCell<Option<RealThreadId>> = RefCell::default();

	// Whether we had to delay preemption checks until the end of a nonpreemptible call.
	static DEFERRED: AtomicBool = AtomicBool::new(false);
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

pub fn enable_preemption(group: Option<Group>) -> Result<Option<Signal>, ()> {
	use timetravel::errno::errno;

	// We must access errno_group() even when we won't use it so that the initial
	// enable_preemption() call bootstraps later resume_preemption() ones!
	let erryes = errno_group(group);
	let errno = *errno();

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

	if DEFERRED.with(|deferred| deferred.swap(false, Ordering::Relaxed)) {
		let signal = unblock.get_or_insert(thread_signal()?);
		drop(pthread_kill(pthread_self(), *signal));
	}

	if let Some(signal) = unblock {
		drop(mask(Operation::Unblock, signal));
	}

	// Propagate any errors encountered during our libc replacements back to the calling libset.
	if group.is_none() && errno != 0 {
		*erryes = errno;
	}
	Ok(unblock)
}

pub fn disable_preemption(block: Option<Signal>) {
	group_thread_set!(Group::SHARED);
	if let Some(signal) = block {
		// Mask the preemption signal without calling thread_signal(), which would by racy.
		drop(mask(Operation::Block, signal));
	}

	SIGNAL.with(|signal| signal.replace(None));
	DEFERRED.with(|deferred| deferred.store(false, Ordering::Relaxed));
}

pub fn is_preemptible() -> bool {
	use gotcha::group_thread_get;

	! group_thread_get!().is_shared()
}

// It is only safe to call this function while preemption is (temporarily) disabled!
pub fn defer_preemption(signum: Option<(&mut Sigset, Signal)>) {
	debug_assert!(! is_preemptible());

	// We must first mask the signal so no attempted preemption races on DEFERRED!
	if let Some((sigmask, signo)) = signum {
		// Caller is asserting we are beneath a signal handler, so we should only update the
		// outside world's mask.
		sigmask.add(signo);
	} else {
		drop(mask(Operation::Block, thread_signal().unwrap()));
	}

	DEFERRED.with(|deferred| deferred.store(true, Ordering::Relaxed));
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

fn errno_group(group: Option<Group>) -> &'static mut c_int {
	use gotcha::group_lookup_symbol_fn;
	use libc::__errno_location;
	thread_local! {
		static ERRNO_LOCATION: RefCell<Option<unsafe extern fn() -> *mut c_int>> =
			RefCell::new(None);
	}

	// We save the location in a thread-local variable so the next time we need to find its
	// *location,* we won't clear its *value* in the process!
	ERRNO_LOCATION.with(|errno_location| {
		let mut errno_location = errno_location.borrow_mut();
		if let Some(group) = group {
			errno_location.replace(unsafe {
				group_lookup_symbol_fn!(group, __errno_location)
			}.unwrap());
		}
		let __errno_location = errno_location.unwrap();
		unsafe {
			&mut *__errno_location()
		}
	})
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

