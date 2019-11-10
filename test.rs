#![crate_type = "lib"]
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
