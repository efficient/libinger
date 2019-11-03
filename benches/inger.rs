#[macro_use]
extern crate bencher;
extern crate inger;

use bencher::Bencher;

benchmark_group![bench, resume];

fn resume(lo: &mut Bencher) {
	use inger::launch;
	use inger::nsnow;
	use inger::pause;
	use inger::resume;
	use std::fs::File;
	use std::io::Write;
	use std::sync::atomic::AtomicBool;
	use std::sync::atomic::AtomicU64;
	use std::sync::atomic::Ordering;

	let run = AtomicBool::from(true);
	let during = AtomicU64::default();
	let mut linger = launch(|| while run.load(Ordering::Relaxed) {
		pause();
		during.store(nsnow(), Ordering::Relaxed);
	}, u64::max_value()).unwrap();

	let mut into = 0;
	let mut outof = 0;
	let mut count = 0;
	lo.iter(|| {
		let before = nsnow();
		resume(&mut linger, u64::max_value()).unwrap();
		let after = nsnow();

		let during = during.load(Ordering::Relaxed);
		into += during - before;
		outof += after - during;
		count += 1;
	});

	if let Ok(mut file) = File::create("bench_resume.log") {
		writeln!(file, "entry resume ... {} ns/iter", into / count).unwrap();
		writeln!(file, "exit  resume ... {} ns/iter", outof / count).unwrap();
		writeln!(file, "(ran for {} iterations)", count).unwrap();
	}

	run.store(false, Ordering::Relaxed);
	resume(&mut linger, u64::max_value()).unwrap();
}

benchmark_main! {
	bench
}
