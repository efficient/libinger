#![feature(asm)]
#![feature(test)]

extern crate test;

use test::Bencher;

#[inline]
unsafe fn nop() {
	// Force the compiler to generate a lazy JUMP_SLOT relocation rather than a GLOB_DAT one.
	asm!("call nop");
}

extern "C" {
	fn nop_location() -> unsafe extern "C" fn();
}

#[bench]
fn eager(lo: &mut Bencher) {
	let nop = unsafe {
		nop_location()
	};
	lo.iter(|| unsafe {
		nop()
	});
}

#[bench]
fn lazy(lo: &mut Bencher) {
	lo.iter(|| unsafe {
		nop()
	});
}
