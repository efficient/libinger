pub use crate::Signal;

use libc::c_int;
use libc::pthread_t;
use std::io::Error;
use std::io::Result;

pub struct PThread (pthread_t);

pub fn pthread_kill(thread: PThread, signal: Signal) -> Result<()> {
	use crate::libgotcha::libgotcha_pthread_kill;

	let code = unsafe {
		libgotcha_pthread_kill(thread.0, signal as c_int)
	};
	if code == 0 {
		Ok(())
	} else {
		Err(Error::from_raw_os_error(code))
	}
}

pub fn pthread_self() -> PThread {
	use libc::pthread_self;

	PThread (unsafe {
		pthread_self()
	})
}
