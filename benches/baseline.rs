#[macro_use]
extern crate bencher;
extern crate libc;

use bencher::Bencher;
use libc::exit;
use libc::waitpid;
use std::ptr::null_mut;

benchmark_group![bench, fork, vfork, pthread_create];

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

fn pthread_create(lo: &mut Bencher) {
	use libc::pthread_create;
	use libc::pthread_join;
	use std::ffi::c_void;
	use std::ptr::null;

	extern fn identity(val: *mut c_void) -> *mut c_void { val }
	lo.iter(|| {
		let mut tid = 0;
		unsafe {
			pthread_create(&mut tid, null(), identity, null_mut());
			pthread_join(tid, null_mut());
		}
	})
}

benchmark_main! {
	bench
}
