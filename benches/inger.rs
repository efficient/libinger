#[macro_use]
extern crate bencher;
extern crate inger;

use bencher::Bencher;

benchmark_group![bench, resume];

fn resume(lo: &mut Bencher) {
	use inger::launch;
	use inger::pause;
	use inger::resume;
	use std::sync::atomic::AtomicBool;
	use std::sync::atomic::Ordering;

	let run = AtomicBool::from(true);
	let mut linger = launch(|| while run.load(Ordering::Relaxed) {
		pause();
	}, u64::max_value()).unwrap();

	lo.iter(||
		drop(resume(&mut linger, u64::max_value()))
	);

	run.store(false, Ordering::Relaxed);
	resume(&mut linger, u64::max_value()).unwrap();
}

benchmark_main! {
	bench
}
