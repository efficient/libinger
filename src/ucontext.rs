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
pub fn makecontext(context: &mut ucontext_t, thunk: extern "C" fn(), stack: &mut [u8], link: Option<&mut ucontext_t>) -> Result<()> {
	use libc::makecontext;

	getcontext(context)?;
	context.uc_stack.ss_sp = stack.as_mut_ptr() as *mut c_void;
	context.uc_stack.ss_size = stack.len();
	if let Some(link) = link {
		context.uc_link = link;
	}
	unsafe {
		makecontext(context, thunk, 0);
	}

	Ok(())
}

#[inline(always)]
pub fn swapcontext(caller: &mut ucontext_t, callee: &mut ucontext_t) -> Result<()> {
	use libc::swapcontext;

	if unsafe {
		swapcontext(caller, callee)
	} == 0 {
		Ok(())
	} else {
		Err(Error::last_os_error())
	}
}

#[cfg(test)]
mod tests {
	use ucontext::*;

	#[test]
	fn double_free() {
		use libc::setcontext;
		use std::cell::RefCell;
		use std::mem::forget;
		use std::rc::Rc;
		use volatile::VolBool;

		let context = Rc::new(RefCell::new(ucontext_t::new()));
		let checker = Rc::downgrade(&context);

		let mut jump = VolBool::new(true);
		{
			let context = context.clone();
			getcontext(&mut context.borrow_mut()).unwrap();
		}

		if jump.get() {
			jump.set(false);
			unsafe {
				setcontext(&mut *context.borrow_mut());
			}
			unreachable!();
		}

		if let None = checker.upgrade() {
			forget(context);
			panic!();
		}
	}
}
