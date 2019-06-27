use libc::ITIMER_PROF;
use libc::ITIMER_REAL;
use libc::ITIMER_VIRTUAL;
pub use libc::itimerval;
pub use libc::timeval;
use std::io::Result;

#[allow(dead_code)]
pub enum Timer {
	Real = ITIMER_REAL as isize,
	Virtual = ITIMER_VIRTUAL as isize,
	Prof = ITIMER_PROF as isize,
}

pub fn setitimer(which: Timer, new: &itimerval, old: Option<&mut itimerval>) -> Result<()> {
	use std::io::Error;
	use std::os::raw::c_int;
	use std::ptr::null_mut;

	extern "C" {
		fn setitimer(which: c_int, new: *const itimerval, old: *mut itimerval) -> c_int;
	}

	if unsafe {
		setitimer(which as i32, new, if let Some(old) = old { old } else { null_mut() })
	} == 0 {
		Ok(())
	} else {
		Err(Error::last_os_error())
	}
}

#[cfg(test)]
mod tests {
	#[test(skip)]
	fn setitimer_oneshot() {
		use libc::siginfo_t;
		use libc::sigsuspend;
		use libc::timeval;
		use libc::ucontext_t;
		use signal::Action;
		use signal::Set;
		use signal::Sigaction;
		use signal::Signal;
		use signal::Sigset;
		use signal::sigaction;

		extern "C" fn handler(_: Signal, _: Option<&siginfo_t>, _: Option<&mut ucontext_t>) {}

		sigaction(Signal::Alarm, &Sigaction::new(handler, Sigset::empty(), 0), None).unwrap();

		setitimer(Timer::Real, &itimerval {
			it_interval: timeval {
				tv_sec: 0,
				tv_usec: 0,
			},
			it_value: timeval {
				tv_sec: 0,
				tv_usec: 10,
			},
		}, None).unwrap();

		let mut mask = Sigset::full();
		mask.del(Signal::Alarm);
		unsafe {
			sigsuspend(&mask);
		}
	}
}
