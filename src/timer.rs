use libc::CLOCK_BOOTTIME;
use libc::CLOCK_BOOTTIME_ALARM;
use libc::CLOCK_MONOTONIC;
use libc::CLOCK_PROCESS_CPUTIME_ID;
use libc::CLOCK_REALTIME;
use libc::CLOCK_REALTIME_ALARM;
use libc::CLOCK_THREAD_CPUTIME_ID;
pub use libc::itimerspec;
use libc::sigevent;
use signal::Signal;
use std::io::Error;
use std::io::Result;
use std::mem::MaybeUninit;
use std::os::raw::c_int;

#[allow(dead_code)]
pub enum Clock {
	Boot = CLOCK_BOOTTIME as _,
	BootAlarm = CLOCK_BOOTTIME_ALARM as _,
	Mono = CLOCK_MONOTONIC as _,
	Process = CLOCK_PROCESS_CPUTIME_ID as _,
	Real = CLOCK_REALTIME as _,
	RealAlarm = CLOCK_REALTIME_ALARM as _,
	Thread = CLOCK_THREAD_CPUTIME_ID as _,
}

#[repr(transparent)]
pub struct Sigevent (sigevent);

impl Sigevent {
	#[allow(dead_code)]
	pub fn none() -> Self {
		use libc::SIGEV_NONE;

		Self (Self::new(SIGEV_NONE))
	}

	#[allow(dead_code)]
	pub fn signal(signal: Signal) -> Self {
		use libc::SIGEV_SIGNAL;

		let mut this = Self::new(SIGEV_SIGNAL);
		this.sigev_signo = signal as _;
		Self (this)
	}

	#[allow(dead_code)]
	pub fn thread_id(signal: Signal, thread: c_int) -> Self {
		use libc::SIGEV_THREAD_ID;

		let mut this = Self::new(SIGEV_THREAD_ID);
		this.sigev_signo = signal as _;
		this.sigev_notify_thread_id = thread;
		Self (this)
	}

	fn new(notify: c_int) -> sigevent {
		let mut event: sigevent = unsafe {
			uninitialized()
		};
		event.sigev_notify = notify;
		event
	}
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Timer (usize);

pub fn timer_create(clockid: Clock, sevp: &mut Sigevent) -> Result<Timer> {
	extern {
		fn timer_create(_: c_int, _: *mut sigevent, _: *mut Timer) -> c_int;
	}

	let mut timer = MaybeUninit::uninit();
	let Sigevent (sevp) = sevp;
	if unsafe {
		timer_create(clockid as _, sevp, timer.as_mut_ptr())
	} != 0 {
		Err(Error::last_os_error())?;
	}

	Ok(unsafe {
		timer.assume_init()
	})
}

pub fn timer_settime(timerid: Timer, absolute: bool, new: &itimerspec, old: Option<&mut itimerspec>) -> Result<()> {
	use libc::TIMER_ABSTIME;
	extern {
		fn timer_settime(_: Timer, _: c_int, _: *const itimerspec, _: Option<&mut itimerspec>) -> c_int;
	}

	let absolute = if absolute { TIMER_ABSTIME } else { 0 };
	if unsafe {
		timer_settime(timerid, absolute, new, old)
	} != 0 {
		Err(Error::last_os_error())?;
	}

	Ok(())
}

#[allow(dead_code)]
pub fn timer_delete(timerid: Timer) -> Result<()> {
	extern {
		fn timer_delete(_: Timer) -> c_int;
	}

	if unsafe {
		timer_delete(timerid)
	} != 0 {
		Err(Error::last_os_error())?;
	}

	Ok(())
}

unsafe fn uninitialized<T>() -> T {
	MaybeUninit::uninit().assume_init()
}
