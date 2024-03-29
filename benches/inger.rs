use bencher::Bencher;
use bencher::benchmark_group;
use bencher::benchmark_main;
use inger::concurrency_limit;
use inger::nsnow;
use inger::pause;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

benchmark_group![bench, launch, resume, renew];

fn launch(lo: &mut Bencher) {
	use inger::STACK_N_PREALLOC;
	use inger::abort;
	use inger::launch;
	use inger::resume;
	use std::mem::MaybeUninit;
	let mut lingers: [MaybeUninit<_>; STACK_N_PREALLOC] = unsafe {
		MaybeUninit::uninit().assume_init()
	};
	let during = AtomicU64::default();

	let mut into = 0;
	let mut outof = 0;
	let mut index = 0;
	let paused = AtomicBool::from(false);
	lo.iter(|| {
		assert!(index < concurrency_limit(), "LIBSETS tunable set too low!");

		let before = nsnow();
		lingers[index] = MaybeUninit::new(launch(|| {
			during.store(nsnow(), Ordering::Relaxed);
			pause();
			if ! paused.load(Ordering::Relaxed) {
				abort("pause() did not context switch!");
			}
		}, u64::max_value()).unwrap());

		let after = nsnow();
		let during = during.load(Ordering::Relaxed);
		into += during - before;
		outof += after - during;

		index += 1;
	});

	if let Ok(mut file) = File::create("bench_launch.log") {
		let index: u64 = index as _;
		writeln!(file, "entry launch ... {} ns/iter", into / index).unwrap();
		writeln!(file, "exit  launch ... {} ns/iter", outof / index).unwrap();
		writeln!(file, "(ran for {} iterations)", index).unwrap();
	}

	paused.store(true, Ordering::Relaxed);
	for linger in &mut lingers[..index] {
		let linger = linger.as_mut_ptr();
		let linger = unsafe {
			&mut *linger
		};
		resume(linger, u64::max_value()).unwrap();
	}
}

fn resume(lo: &mut Bencher) {
	use inger::launch;
	use inger::resume;

	let run = AtomicBool::from(true);
	let during = AtomicU64::default();
	let mut lingers: Vec<_> = (0..concurrency_limit()).map(|_| launch(|| while run.load(Ordering::Relaxed) {
		pause();
		during.store(nsnow(), Ordering::Relaxed);
	}, u64::max_value()).unwrap()).collect();
	let nlingers = lingers.len();

	let mut into = 0;
	let mut outof = 0;
	let mut index = 0;
	lo.iter(|| {
		let before = nsnow();
		resume(&mut lingers[index % nlingers], u64::max_value()).unwrap();

		let after = nsnow();
		let during = during.load(Ordering::Relaxed);
		into += during - before;
		outof += after - during;

		index += 1;
	});

	if let Ok(mut file) = File::create("bench_resume.log") {
		let index: u64 = index as _;
		writeln!(file, "entry resume ... {} ns/iter", into / index).unwrap();
		writeln!(file, "exit  resume ... {} ns/iter", outof / index).unwrap();
		writeln!(file, "(ran for {} iterations)", index).unwrap();
	}

	run.store(false, Ordering::Relaxed);
	for linger in &mut lingers {
		resume(linger, u64::max_value()).unwrap();
	}
}

fn renew(lo: &mut Bencher) {
	use inger::launch;

	let lingers: Vec<_> = (0..concurrency_limit()).map(|_| launch(pause, u64::max_value()).unwrap()).collect();
	let mut lingers = lingers.into_iter();
	lo.iter(||
		drop(lingers.next().expect("LIBSETS tunable set too low!"))
	);
}

benchmark_main! {
	bench
}
