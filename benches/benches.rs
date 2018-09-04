#![feature(test)]

extern crate libc;
extern crate test;
extern crate timetravel;

use libc::MINSIGSTKSZ;
use libc::ucontext_t;
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

fn get_helper<T, F: FnMut(ucontext_t) -> T>(lo: &mut Bencher, mut fun: F) {
	use libc::getcontext;

	lo.iter(|| {
		let mut initial = true;
		unsafe {
			let mut context = uninitialized();
			getcontext(&mut context);
			if read_volatile(&initial) {
				write_volatile(&mut initial, false);
				fun(context);
			}
		}
	});
}

#[bench]
fn getset_native(lo: &mut Bencher) {
	use libc::setcontext;

	get_helper(lo, |context| unsafe {
		setcontext(&context)
	});
}

#[bench]
fn getset_timetravel(lo: &mut Bencher) {
	use timetravel::getcontext;
	use timetravel::setcontext;

	lo.iter(|| getcontext(|context| setcontext(&context), || None));
}

fn make_helper<T, F: FnMut(ucontext_t) -> T>(lo: &mut Bencher, mut fun: F) {
	use libc::getcontext;
	use libc::makecontext;

	extern "C" fn stub() {}

	let mut stack = [0u8; MINSIGSTKSZ];
	get_helper(lo, |mut context| {
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
		fun(gate);
	});
}

#[bench]
fn make_native(lo: &mut Bencher) {
	make_helper(lo, |_| ());
}

#[bench]
fn make_timetravel(lo: &mut Bencher) {
	use timetravel::makecontext;

	let mut stack = [0u8; MINSIGSTKSZ];
	lo.iter(|| makecontext(&mut stack[..], |_| (), || ()));
}

#[bench]
fn makeset_native(lo: &mut Bencher) {
	use libc::setcontext;

	make_helper(lo, |gate| unsafe {
		setcontext(&gate)
	});
}

#[bench]
fn makeset_timetravel(lo: &mut Bencher) {
	use timetravel::makecontext;
	use timetravel::setcontext;

	let mut stack = [0u8; MINSIGSTKSZ];
	lo.iter(|| makecontext(&mut stack[..], |gate| panic!(setcontext(&gate)), || ()));
}
