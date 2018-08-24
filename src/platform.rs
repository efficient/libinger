use invar::MoveInvariant;
use libc::ucontext_t;
pub use self::imp::*;
use uninit::Uninit;

pub trait Link {
	fn link(&self) -> &'static mut *mut Self;
}

#[cfg(target_os = "linux")]
mod imp {
	use self::regs::*;
	use super::*;

	#[cfg(target_arch = "x86_64")]
	mod regs {
		#[allow(dead_code)]
		pub const NGREG: usize = 23;
		pub const REG_RBX: usize = 11;
	}
	#[cfg(not(target_arch = "x86_64"))]
	compile_error!("registers not defined for this target architecture");

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

	impl Link for ucontext_t {
		#[inline]
		fn link(&self) -> &'static mut *mut Self {
			use std::mem::transmute;

			let link = self.uc_mcontext.gregs[REG_RBX];
			unsafe {
				transmute(link)
			}
		}
	}
}

#[cfg(not(target_os = "linux"))]
mod imp {
	use super::*;

	unsafe impl Uninit for ucontext_t {}
	impl MoveInvariant for ucontext_t {}

	impl Link for ucontext_t {
		fn link(&self) -> &'static mut *mut Self {
			unimplemented!()
		}
	}
}
