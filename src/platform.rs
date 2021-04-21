use crate::invar::MoveInvariant;
use crate::swap::Swap;
use crate::uninit::Uninit;
use crate::zero::Zero;
pub use self::imp::*;

use libc::sigset_t;
use libc::ucontext_t;

pub trait Stack {
	fn stack_ptr(&self) -> usize;
}

pub trait Link {
	fn link(&self) -> &'static mut *mut Self;
}

unsafe impl Zero for sigset_t {}

#[cfg(target_os = "linux")]
mod imp {
	use self::regs::*;
	use super::*;

	#[cfg(target_arch = "x86_64")]
	mod regs {
		#[allow(dead_code)]
		pub const NGREG: usize = 23;
		pub const REG_CSGSFS: usize = 18;
		pub const REG_RBX: usize = 11;
		pub const REG_RSP: usize = 15;
	}
	#[cfg(not(target_arch = "x86_64"))]
	compile_error!("registers not defined for this target architecture");

	unsafe impl Uninit for ucontext_t {
		#[inline]
		fn uninit() -> Self {
			use libc::_libc_fpstate;
			use libc::greg_t;
			use libc::stack_t;
			use std::mem::MaybeUninit;
			use std::mem::transmute;
			use std::os::raw::c_ulong;
			use std::ptr::write;

			unsafe impl Zero for [greg_t; NGREG] {}

			#[repr(C)]
			struct UninitUcontextT {
				uc_flags: MaybeUninit<c_ulong>,
				uc_link: MaybeUninit<*mut ucontext_t>,
				uc_stack: MaybeUninit<stack_t>,
				uc_mcontext: UninitMcontextT,
				uc_sigmask: MaybeUninit<sigset_t>,
				_buf: MaybeUninit<[u8; 512]>,
			}

			#[repr(C)]
			struct UninitMcontextT {
				gregs: MaybeUninit<[greg_t; NGREG]>,
				fpgregs: MaybeUninit<*mut _libc_fpstate>,
				_buf: MaybeUninit<[u8; 64]>,
			}

			let mut this = UninitUcontextT {
				uc_flags: MaybeUninit::uninit(),
				uc_link: MaybeUninit::uninit(),
				uc_stack: MaybeUninit::uninit(),
				uc_mcontext: UninitMcontextT {
					gregs: MaybeUninit::uninit(),
					fpgregs: MaybeUninit::uninit(),
					_buf: MaybeUninit::uninit(),
				},
				uc_sigmask: MaybeUninit::uninit(),
				_buf: MaybeUninit::uninit(),
			};
			unsafe {
				write(this.uc_mcontext.gregs.as_mut_ptr(), Zero::zero());
				transmute(this)
			}
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

	impl Swap for ucontext_t {
		type Other = Self;

		fn swap(&mut self, other: &mut Self::Other) -> bool {
			use std::mem::swap;

			self.after_move();
			swap(&mut self.uc_mcontext, &mut other.uc_mcontext);
			swap(&mut self.uc_mcontext.gregs[REG_CSGSFS], &mut other.uc_mcontext.gregs[REG_CSGSFS]);

			let self_fp = unsafe {
				&mut *self.uc_mcontext.fpregs
			};
			let other_fp = unsafe {
				&mut *other.uc_mcontext.fpregs
			};
			swap(self_fp, other_fp);
			swap(&mut self.uc_mcontext.fpregs, &mut other.uc_mcontext.fpregs);

			swap(&mut self.uc_flags, &mut other.uc_flags);
			swap(&mut self.uc_link, &mut other.uc_link);
			swap(&mut self.uc_stack, &mut other.uc_stack);
			swap(&mut self.uc_sigmask, &mut other.uc_sigmask);

			true
		}
	}

	impl Stack for ucontext_t {
		#[inline]
		fn stack_ptr(&self) -> usize {
			self.uc_mcontext.gregs[REG_RSP] as _
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

	impl Swap for ucontext_t {
		type Other = Self;

		fn swap(&mut self, other: &Self::Other) -> bool {
			unimplemented!()
		}
	}

	impl Stack for ucontext_t {
		fn stack_ptr(&self) -> usize {
			unimplemented!()
		}
	}

	impl Link for ucontext_t {
		fn link(&self) -> &'static mut *mut Self {
			unimplemented!()
		}
	}
}
