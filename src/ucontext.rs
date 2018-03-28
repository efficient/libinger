use libc::c_void;
use libc::ucontext_t;
use std::io::Error;
use std::io::Result;
use volatile::VolBool;
use zeroable::Zeroable;

unsafe impl Zeroable for ucontext_t {}

pub const REG_CSGSFS: usize = 18;

// This must be inlined because it stack-allocates a volatile bool that it expects to be present
// even after `getcontext()` returns for the second (or subsequent) time!
#[inline(always)]
pub fn getcontext() -> Result<Option<ucontext_t>> {
	use libc::getcontext;
	use std::mem::forget;

	let mut context = ucontext_t::new();
	let mut creating = VolBool::new(true);
	if unsafe {
		getcontext(&mut context)
	} == 0 {
		Ok(if creating.get() {
			creating.set(false);
			Some(context)
		} else {
			forget(context);
			forget(creating);
			None
		})
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
	fn getcontext_invoke() {
		use libc::setcontext;

		let mut reached = VolBool::new(false);
		if let Some(mut context) = getcontext().unwrap() {
			assert!(! reached.get());
			reached.set(true);
			unsafe {
				setcontext(&mut context);
			}
			unreachable!();
		}
		assert!(reached.get());
	}
}
