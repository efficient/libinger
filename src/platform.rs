use invar::MoveInvariant;
use libc::ucontext_t;
pub use self::imp::*;
use uninit::Uninit;

#[cfg(target_os = "linux")]
mod imp {
	use super::*;

	#[cfg(target_arch = "x86_64")]
	const NGREG: usize = 23;
	#[cfg(not(target_arch = "x86_64"))]
	compile_error!("NGREG not defined for this target architecture");

	unsafe impl Uninit for ucontext_t {
		#[inline]
		fn uninit() -> Self {
			use libc::greg_t;
			use std::mem::uninitialized;
			use std::ptr::write;
			use zero::Zero;

			unsafe impl Zero for [greg_t; NGREG] {}

			let mut this: Self;
			unsafe {
				this = uninitialized();
				write(&mut this.uc_mcontext.gregs, Zero::zero());
			}

			this
		}
	}

	impl MoveInvariant for ucontext_t {
		#[inline]
		fn after_move(&mut self) {
			use libc::_libc_fpstate;

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

	unsafe impl Uninit for ucontext_t {}
	impl MoveInvariant for ucontext_t {}
}
