#[macro_use]
extern crate bencher;

use bencher::Bencher;

benchmark_group![bench, fork];

fn fork(lo: &mut Bencher) {
	use std::os::raw::c_int;
	extern {
		fn exit(_: c_int) -> !;
		fn fork() -> c_int;
		fn waitpid(_: c_int, _: usize, _: c_int);
	}

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
			waitpid(pid, 0, 0);
		}
	})
}

benchmark_main! {
	bench
}
