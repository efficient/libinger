use libc::ucontext_t;
use std::io::Error;
use std::io::Result;

const REG_CSGSFS: usize = 18;

pub fn cpycontext(dest: &mut ucontext_t, src: &ucontext_t) {
	dest.uc_stack = src.uc_stack;

	let segs = dest.uc_mcontext.gregs[REG_CSGSFS];
	dest.uc_mcontext = src.uc_mcontext;
	dest.uc_mcontext.gregs[REG_CSGSFS] = segs;
}

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
