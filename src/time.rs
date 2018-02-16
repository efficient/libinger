use libc::ITIMER_PROF;
use libc::ITIMER_REAL;
use libc::ITIMER_VIRTUAL;
pub use libc::itimerval;
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
