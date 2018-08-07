use invar::MoveInvariant;
use libc::ucontext_t;
pub use self::imp::*;

#[cfg(target_os = "linux")]
mod imp {
	use libc::_libc_fpstate;
	use super::*;

	impl MoveInvariant for ucontext_t {
		fn after_move(&mut self) {
			let start = self as *mut ucontext_t;
			let end = unsafe {
				start.add(1)
			} as *mut _libc_fpstate;
			self.uc_mcontext.fpregs = unsafe {
				end.sub(1)
			};
		}
	}
}

#[cfg(not(target_os = "linux"))]
mod imp {
	use super::*;

	impl MoveInvariant for ucontext_t {}
}
