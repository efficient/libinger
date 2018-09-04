#![feature(test)]

extern crate libc;
extern crate test;
extern crate timetravel;

use libc::MINSIGSTKSZ;
use std::mem::uninitialized;
use std::ptr::read_volatile;
use std::ptr::write_volatile;
use test::Bencher;

#[bench]
fn get_native(lo: &mut Bencher) {
	use libc::getcontext;

	lo.iter(|| unsafe {
		getcontext(&mut uninitialized());
	});
}

#[bench]
fn get_timetravel(lo: &mut Bencher) {
	use timetravel::getcontext;

	lo.iter(|| getcontext(|_| (), || ()));
}

#[bench]
fn getset_native(lo: &mut Bencher) {
	use libc::getcontext;
	use libc::setcontext;

	lo.iter(|| {
		let mut initial = true;
		unsafe {
			let mut context = uninitialized();
			getcontext(&mut context);
			if read_volatile(&initial) {
				write_volatile(&mut initial, false);
				setcontext(&context);
			}
		}
	});
}

#[bench]
fn getset_timetravel(lo: &mut Bencher) {
	use timetravel::getcontext;
	use timetravel::setcontext;

	lo.iter(|| getcontext(|context| setcontext(&context), || None));
}

#[bench]
fn make_native(lo: &mut Bencher) {
	use libc::getcontext;
	use libc::makecontext;

	extern "C" fn stub() {}

	let mut stack = [0u8; MINSIGSTKSZ];
	lo.iter(|| {
		let mut initial = true;
		let mut context = unsafe {
			uninitialized()
		};
		unsafe {
			getcontext(&mut context);
		}
		if unsafe {
			read_volatile(&initial)
		} {
			unsafe {
				write_volatile(&mut initial, false);
			}
			let mut gate = unsafe {
				uninitialized()
			};
			unsafe {
				getcontext(&mut gate);
			}
			gate.uc_stack.ss_sp = stack.as_mut_ptr() as _;
			gate.uc_stack.ss_size = stack.len();
			gate.uc_link = &mut context;
			unsafe {
				makecontext(&mut gate, stub, 0);
			}
		}
	});
}

#[bench]
fn make_timetravel(lo: &mut Bencher) {
	use timetravel::makecontext;

	let mut stack = [0u8; MINSIGSTKSZ];
	lo.iter(|| makecontext(&mut stack[..], |_| (), || ()));
}
