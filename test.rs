#![crate_type = "lib"]
#![feature(asm)]
#![feature(core_intrinsics)]
#![feature(staged_api)]
#![feature(test)]
#![stable(feature = "test", since = "0")]

extern crate test;

#[stable(feature = "test", since = "0")]
pub use test::*;

#[stable(feature = "test", since = "0")]
pub trait Bencher {
	#[stable(feature = "test", since = "0")]
	fn iter<T>(&mut self, _: impl FnMut() -> T);
}

#[stable(feature = "test", since = "0")]
impl Bencher for test::Bencher {
	fn iter<T>(&mut self, fun: impl FnMut() -> T) {
		self.iter(fun);
	}
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
	#[stable(feature = "test", since = "0")]
	pub fn nop();
}
