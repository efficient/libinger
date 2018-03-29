use libc::ucontext_t;
use std::io::Error;
use std::io::Result;
use volatile::VolBool;
use zeroable::Zeroable;

pub enum Void {}

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

pub fn setcontext(context: &ucontext_t) -> Result<Void> {
	use libc::setcontext;

	unsafe {
		setcontext(context);
	}
	Err(Error::last_os_error())
}

pub fn makecontext(thunk: extern "C" fn(), stack: &mut [u8], link: Option<&mut ucontext_t>) -> Result<ucontext_t> {
	use libc::c_void;
	use libc::makecontext;

	let mut context = getcontext()?.unwrap();
	context.uc_stack.ss_sp = stack.as_mut_ptr() as *mut c_void;
	context.uc_stack.ss_size = stack.len();
	if let Some(link) = link {
		context.uc_link = link;
	}

	unsafe {
		makecontext(&mut context, thunk, 0);
	}

	Ok(context)
}

unsafe impl Zeroable for ucontext_t {}

#[cfg(test)]
mod tests {
	use std::cell::Cell;
	use std::os::raw::c_void;
	use ucontext::*;

	thread_local! {
		static DROP_COUNT: Cell<Option<i32>> = Cell::new(None);
	}

	#[test]
	fn getcontext_invoke() {
		let mut reached = VolBool::new(false);
		if let Some(context) = getcontext().unwrap() {
			assert!(! reached.get());
			reached.set(true);
			setcontext(&context).unwrap();
			unreachable!();
		}
		assert!(reached.get());
	}

	#[test]
	fn makecontext_invoke() {
		thread_local! {
			static REACHED: Cell<bool> = Cell::new(false);
		}

		extern "C" fn callback() {
			REACHED.with(|reached| reached.set(true));
		}

		let mut stack = vec![0; 1_024];
		if let Some(mut here) = getcontext().unwrap() {
			let mut there = makecontext(callback, &mut stack, Some(&mut here)).unwrap();
			setcontext(&there).unwrap();
			unreachable!();
		}
		assert!(REACHED.with(|reached| reached.get()));
	}

	#[test]
	fn address_change_groundtruth() {
		use libc::getcontext;

		let mut context = ucontext_t::new();
		unsafe {
			getcontext(&mut context);
		}

		let mcontext: *const _ = context.uc_mcontext.fpregs;
		let mcontext = unsafe {
			mcontext.offset(1)
		} as *const c_void;

		let context: *const _ = &context;
		let context = unsafe {
			context.offset(1)
		} as *const c_void;

		assert_eq!(mcontext, context);
	}

	#[test]
	fn address_change_getcontext() {
		let context = getcontext().unwrap().unwrap();

		let mcontext: *const _ = context.uc_mcontext.fpregs;
		let mcontext = unsafe {
			mcontext.offset(1)
		} as *const c_void;

		let context: *const _ = &context;
		let context = unsafe {
			context.offset(1)
		} as *const c_void;

		assert_eq!(mcontext, context);
	}

	#[test]
	fn address_change_makecontext() {
		extern "C" fn callback() {}

		let context = makecontext(callback, &mut [], None).unwrap();

		let mcontext: *const _ = context.uc_mcontext.fpregs;
		let mcontext = unsafe {
			mcontext.offset(1)
		} as *const c_void;

		let context: *const _ = &context;
		let context = unsafe {
			context.offset(1)
		} as *const c_void;

		assert_eq!(mcontext, context);
	}

	#[test]
	fn double_free_uninvoked() {
		DROP_COUNT.with(|drop_count| drop_count.set(Some(1)));
		getcontext().unwrap();
		assert!(DROP_COUNT.with(|drop_count| drop_count.get()).unwrap() == 0);
	}

	#[test]
	fn double_free_invoked() {
		DROP_COUNT.with(|drop_count| drop_count.set(Some(1)));
		if let Some(mut context) = getcontext().unwrap() {
			setcontext(&context).unwrap();
			unreachable!();
		}
		assert!(DROP_COUNT.with(|drop_count| drop_count.get()).unwrap() == 0);
	}

	impl Drop for VolBool {
		fn drop(&mut self) {
			DROP_COUNT.with(|drop_count| if let Some(enforcing) = drop_count.get() {
				if enforcing == 0 {
					drop_count.set(None);
					panic!();
				} else {
					drop_count.set(Some(enforcing - 1));
				}
			});
		}
	}
}
