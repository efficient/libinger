use libc::ucontext_t;
use std::io::Error;
use std::io::Result;
use zeroable::Zeroable;

unsafe impl Zeroable for ucontext_t {}

pub const REG_CSGSFS: usize = 18;

pub fn getcontext(context: &mut ucontext_t) -> Result<()> {
	use libc::getcontext;

	if unsafe {
		getcontext(context)
	} == 0 {
		Ok(())
	} else {
		Err(Error::last_os_error())
	}
}
