extern crate test;

use test::Bencher;
use test::nop;

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
fn eager(lo: &mut impl Bencher) {
	with_eager_nop(|| lo.iter(|| unsafe {
		nop()
	}));
}

#[bench]
fn lazy(lo: &mut impl Bencher) {
	lo.iter(|| unsafe {
		nop()
	});
}
