#![feature(asm)]
#![feature(test)]

extern crate test;

use test::Bencher;

#[inline]
unsafe fn nop() {
	// Force the compiler to generate a lazy JUMP_SLOT relocation rather than a GLOB_DAT one.
	asm!("call nop");
}

fn with_eager_nop<T: FnMut()>(mut fun: T) {
	use std::mem::transmute;

	extern {
		fn with_eager_nop(fun: extern fn());
	}

	static mut FUN: Option<*mut dyn FnMut()> = None;

	extern fn adapter() {
		let fun = unsafe {
			&mut *FUN.take().unwrap()
		};
		fun();
	}

	let fun: &mut dyn FnMut() = &mut fun;
	unsafe {
		FUN.replace(transmute(fun));
		with_eager_nop(adapter);
	}
}

#[bench]
fn eager(lo: &mut Bencher) {
	with_eager_nop(|| lo.iter(|| unsafe {
		nop()
	}));
}

#[bench]
fn lazy(lo: &mut Bencher) {
	lo.iter(|| unsafe {
		nop()
	});
}
