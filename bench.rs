#![feature(test)]

extern crate test;

use test::Bencher;

extern "C" {
	fn nop();
}

#[bench]
fn eager(lo: &mut Bencher) {
	lo.iter(|| unsafe {
		nop()
	});
}
