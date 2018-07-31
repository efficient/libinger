use libc::ucontext_t;
use std::io::Error;
use std::io::Result;
use zeroable::Zeroable;

pub enum Void {}

pub const REG_CSGSFS: usize = 18;

pub fn getcontext() -> Result<Option<ucontext_t>> {
	use libc::getcontext;
	use std::mem::forget;

	let mut context = ucontext_t::new();
	if unsafe {
		getcontext(&mut context)
	} == 0 {
		// We co-opt this field to indicate whether we're entering this function for the
		// first or a subsequent time.  This still works for contexts with their own stacks,
		// in which case ss_size is already initialized to something nonzero *after* the
		// first call to getcontext().
		Ok(if context.uc_stack.ss_size == 0 {
			context.uc_stack.ss_size = 1;
			Some(context)
		} else {
			forget(context);
			None
		})
	} else {
		Err(Error::last_os_error())
	}
}

pub fn setcontext(context: &mut ucontext_t) -> Result<Void> {
	use libc::setcontext;

	fixupcontext(context);
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

pub fn sigsetcontext(context: &ucontext_t) -> Result<Void> {
	use libc::siginfo_t;
	use pthread::pthread_kill;
	use pthread::pthread_self;
	use signal::Action;
	use signal::Set;
	use signal::Sigaction;
	use signal::Signal;
	use signal::Sigset;
	use signal::sigaction;
	use std::cell::Cell;
	use std::cell::RefCell;

	extern "C" fn handler(_: Signal, _: Option<&siginfo_t>, context: Option<&mut ucontext_t>) {
		HANDLER.with(|handler|
			sigaction(Signal::VirtualAlarm, &*handler.borrow(), None)
		).unwrap();

		let context = context.unwrap();
		let mut protext = CONTEXT.with(|context| context.take()).unwrap();
		fixupcontext(&mut protext);

		let segs = protext.uc_mcontext.gregs[REG_CSGSFS];
		let fpregs = unsafe {
			context.uc_mcontext.fpregs.as_mut()
		}.unwrap();

		context.uc_flags = protext.uc_flags;
		context.uc_link = protext.uc_link;
		context.uc_stack = protext.uc_stack;
		context.uc_mcontext = protext.uc_mcontext;
		context.uc_mcontext.gregs[REG_CSGSFS] = segs;
		context.uc_mcontext.fpregs = fpregs;
		*fpregs = *unsafe {
			protext.uc_mcontext.fpregs.as_ref()
		}.unwrap();
		context.uc_sigmask = protext.uc_sigmask;
	}

	thread_local! {
		static CONTEXT: Cell<Option<ucontext_t>> = Cell::new(None);
		static HANDLER: RefCell<Sigaction> =
			RefCell::new(Sigaction::new(handler, Sigset::empty(), 0));
	}

	CONTEXT.with(|protext| protext.set(Some(context.clone())));

	let handle = Sigaction::new(handler, Sigset::empty(), 0);
	HANDLER.with(|handlee|
		sigaction(Signal::VirtualAlarm, &handle, Some(&mut handlee.borrow_mut()))
	)?;

	pthread_kill(pthread_self(), Signal::VirtualAlarm)?;

	Err(Error::last_os_error())
}

/// Note that this function assumes that both contexts' `fpregs` pointers are correct!
///
/// If this is not the case (e.g. because either or both have been memmove()'d since creation), be
/// sure to first call `fixupcontext()` on the affected one(s)!
pub fn swap(left: &mut ucontext_t, right: &mut ucontext_t) {
	use std::mem::swap;

	let l_fpregs = unsafe {
		left.uc_mcontext.fpregs.as_mut()
	};
	let r_fpregs = unsafe {
		right.uc_mcontext.fpregs.as_mut()
	};

	assert!(l_fpregs.is_none() == r_fpregs.is_none());
	if let (Some(l_fpregs), Some(r_fpregs)) = (l_fpregs, r_fpregs) {
		swap(l_fpregs, r_fpregs);
		swap(&mut left.uc_mcontext.fpregs, &mut right.uc_mcontext.fpregs);
	}

	// We intentionally skip swapping the uc_stack s because signal handlers receive a special
	// "default" stack that we don't want to save.
	swap(&mut left.uc_flags, &mut right.uc_flags);
	swap(&mut left.uc_link, &mut right.uc_link);
	swap(&mut left.uc_mcontext, &mut right.uc_mcontext);
	swap(&mut left.uc_sigmask, &mut right.uc_sigmask);
}

pub fn fixupcontext(context: &mut ucontext_t) {
	use libc::_libc_fpstate;

	let ptr: *mut _ = context;
	let ptr = unsafe {
		ptr.offset(1)
	} as *mut _libc_fpstate;
	context.uc_mcontext.fpregs = unsafe {
		ptr.offset(-1)
	};
}

unsafe impl Zeroable for ucontext_t {}

#[cfg(test)]
mod tests {
	use std::cell::Cell;
	use std::os::raw::c_void;
	use super::*;
	use volatile::VolBool;

	thread_local! {
		static DROP_COUNT: Cell<Option<i32>> = Cell::new(None);
	}

	#[test]
	fn getcontext_invoke() {
		let mut reached = VolBool::new(false);
		if let Some(mut context) = getcontext().unwrap() {
			assert!(! reached.get());
			reached.set(true);
			setcontext(&mut context).unwrap();
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
			setcontext(&mut there).unwrap();
			unreachable!();
		}
		assert!(REACHED.with(|reached| reached.get()));
	}

	#[test]
	fn fixup_invariant() {
		use libc::getcontext;

		let mut context = ucontext_t::new();
		unsafe {
			getcontext(&mut context);
		}

		assert_eq!(addr_of_end(&context), addr_of_end(context.uc_mcontext.fpregs));
		fixupcontext(&mut context);
		assert_eq!(addr_of_end(&context), addr_of_end(context.uc_mcontext.fpregs));
	}

	#[test]
	fn fixup_getcontext() {
		let mut context = getcontext().unwrap().unwrap();
		fixupcontext(&mut context);
		assert_eq!(addr_of_end(&context), addr_of_end(context.uc_mcontext.fpregs));
	}

	#[test]
	fn fixup_makecontext() {
		extern "C" fn callback() {}

		let mut context = makecontext(callback, &mut [0; 1_024], None).unwrap();
		fixupcontext(&mut context);
		assert_eq!(addr_of_end(&context), addr_of_end(context.uc_mcontext.fpregs));
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
			setcontext(&mut context).unwrap();
			unreachable!();
		}
		assert!(DROP_COUNT.with(|drop_count| drop_count.get()).unwrap() == 0);
	}

	fn addr_of_end<T>(beginning: *const T) -> *const c_void {
		let beginning: *const _ = beginning;
		unsafe {
			beginning.offset(1) as *const _
		}
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
