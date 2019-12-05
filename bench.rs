extern crate test;

use std::os::raw::c_int;
use test::Bencher;
use test::nop;

extern {
	fn free(_: usize);
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

fn in_ancillary_group<T: FnMut()>(mut fun: T) {
	use std::sync::ONCE_INIT;
	use std::sync::Once;
	extern {
		fn libgotcha_group_new();
		fn libgotcha_group_thread_accessor() -> unsafe extern fn(i64);
	}

	static INIT: Once = ONCE_INIT;
	INIT.call_once(|| unsafe {
		libgotcha_group_new();
	});
	unsafe {
		libgotcha_group_thread_accessor()(1);
	}
	fun();
	unsafe {
		libgotcha_group_thread_accessor()(0);
	}
}

#[bench]
fn eager(lo: &mut impl Bencher) {
	with_eager_nop(|| lazy(lo));
}

#[bench]
fn lazy(lo: &mut impl Bencher) {
	lo.iter(|| unsafe {
		nop()
	});
}

#[bench]
fn whitelist(lo: &mut impl Bencher) {
	in_ancillary_group(|| lo.iter(|| unsafe {
		free(0)
	}));
}

#[bench]
fn hook(lo: &mut impl Bencher) {
	extern {
		fn libgotcha_shared_hook(_: Option<extern fn()>);
	}

	extern fn callback() {}
	unsafe {
		libgotcha_shared_hook(Some(callback));
	}
	whitelist(lo);
	unsafe {
		libgotcha_shared_hook(None);
	}
}

#[bench]
fn global(lo: &mut impl Bencher) {
	use std::ptr::read_volatile;
	use test::black_box;
	extern {
		static no: bool;
	}

	lo.iter(|| unsafe {
		read_volatile(black_box(&no))
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
