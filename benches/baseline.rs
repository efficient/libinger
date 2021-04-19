use bencher::Bencher;
use bencher::benchmark_group;
use bencher::benchmark_main;
use libc::exit;
use libc::waitpid;
use std::ffi::c_void;
use std::ptr::null;
use std::ptr::null_mut;

benchmark_group![bench, fork, vfork, pthread_create, pthread_join];

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

	lo.iter(|| {
		let mut tid = 0;
		unsafe {
			pthread_create(&mut tid, null(), identity, null_mut());
			pthread_join(tid, null_mut());
		}
	})
}

fn pthread_join(lo: &mut Bencher) {
	use libc::pthread_create;
	use libc::pthread_join;
	use std::collections::VecDeque;

	let mut tids: VecDeque<_> = (0..100_000).map(|_| {
		let mut tid = 0;
		unsafe {
			pthread_create(&mut tid, null(), identity, null_mut())
		};
		tid
	}).collect();
	lo.iter(|| {
		let tid = tids.pop_front().unwrap();
		unsafe {
			pthread_join(tid, null_mut());
		}
	});
}

extern fn identity(val: *mut c_void) -> *mut c_void { val }

benchmark_main! {
	bench
}
