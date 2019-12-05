extern crate test;

use std::os::raw::c_int;
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

#[bench]
fn gettimeofday(lo: &mut impl Bencher) {
	use test::Timeval;
	extern {
		fn gettimeofday(_: Option<&mut Timeval>, _: usize) -> c_int;
	}

	let mut tv = Timeval::default();
	lo.iter(|| unsafe {
		gettimeofday(Some(&mut tv), 0)
	});
}

#[bench]
fn getpid(lo: &mut impl Bencher) {
	extern {
		fn getpid() -> c_int;
	}

	lo.iter(|| unsafe {
		getpid()
	});
}
