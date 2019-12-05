#[macro_use]
extern crate bencher;
extern crate libc;

use bencher::Bencher;
use libc::exit;
use libc::waitpid;
use std::ptr::null_mut;

benchmark_group![bench, fork, vfork];

fn fork(lo: &mut Bencher) {
	use libc::fork;

	lo.iter(|| {
		let pid = unsafe {
			fork()
		};
		if pid == 0 {
			unsafe {
				exit(0);
			}
		}
		unsafe {
			waitpid(pid, null_mut(), 0);
		}
	})
}

fn vfork(lo: &mut Bencher) {
	use libc::vfork;

	lo.iter(|| {
		let pid = unsafe {
			vfork()
		};
		if pid == 0 {
			unsafe {
				exit(0);
			}
		}
		unsafe {
			waitpid(pid, null_mut(), 0);
		}
	})
}

benchmark_main! {
	bench
}
