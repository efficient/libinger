#![crate_type = "lib"]
#![feature(asm)]
#![feature(rustc_private)]
#![feature(test)]

extern crate libc;
extern crate test;

use libc::timeval;
pub use test::*;

#[repr(transparent)]
pub struct Timeval (timeval);

impl Default for Timeval {
	fn default() -> Self {
		Self (timeval {
			tv_sec: 0,
			tv_usec: 0,
		})
	}
}

pub trait Bencher {
	fn iter<T>(&mut self, _: impl FnMut() -> T);
}

impl Bencher for test::Bencher {
	fn iter<T>(&mut self, fun: impl FnMut() -> T) {
		self.iter(fun);
	}
}

#[inline]
pub fn black_box<T>(t: T) -> T {
	use test::black_box;
	black_box(t)
}

/// Like extern, but calls generate lazy JUMP_SLOT relocations rather than eager GLOB_DAT ones.
macro_rules! lazy_extern {
	($($(#[$attr:meta])* $vis:vis fn $fun:ident($($argv:tt: $argt:ty),*);)*) => {$(
		$(#[$attr])*
		$vis unsafe extern fn $fun($($argv: $argt),*) {
			asm!(concat!("call ", stringify!($fun)));
		}
	)*}
}

lazy_extern! {
	#[inline]
	pub fn nop();
}
