use libc::c_void;
use libc::ucontext_t;
use std::io::Error;
use std::io::Result;
use zeroable::Zeroable;

unsafe impl Zeroable for ucontext_t {}

pub const REG_CSGSFS: usize = 18;

#[inline(always)]
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

#[inline(always)]
pub fn makecontext(context: &mut ucontext_t, thunk: extern "C" fn(), stack: &mut [u8]) -> Result<()> {
	use libc::makecontext;

	getcontext(context)?;
	context.uc_stack.ss_sp = stack.as_mut_ptr() as *mut c_void;
	context.uc_stack.ss_size = stack.len();
	unsafe {
		makecontext(context, thunk, 0);
	}

	Ok(())
}

#[inline(always)]
pub fn swapcontext(link: &mut ucontext_t, context: &mut ucontext_t) -> Result<()> {
	use libc::swapcontext;

	context.uc_link = link;

	if unsafe {
		swapcontext(link, context)
	} == 0 {
		Ok(())
	} else {
		Err(Error::last_os_error())
	}
}
