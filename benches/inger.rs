#[macro_use]
extern crate bencher;
extern crate inger;

use bencher::Bencher;
use inger::STACK_N_PREALLOC;
use inger::nsnow;
use inger::pause;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

const LIBSETS: usize = STACK_N_PREALLOC;

benchmark_group![bench, launch, resume, renew];

fn launch(lo: &mut Bencher) {
	use inger::launch;
	use inger::resume;
	use std::mem::MaybeUninit;
	let mut lingers: [MaybeUninit<_>; LIBSETS] = unsafe {
		MaybeUninit::uninit().assume_init()
	};
	let during = AtomicU64::default();

	let mut into = 0;
	let mut outof = 0;
	let mut index = 0;
	lo.iter(|| {
		assert!(index < lingers.len(), "LIBSETS tunable set too low!");

		let before = nsnow();
		lingers[index] = MaybeUninit::new(launch(|| {
			during.store(nsnow(), Ordering::Relaxed);
			pause();
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

	let during = AtomicU64::default();
	let mut lingers: Vec<_> = (0..LIBSETS).map(|_| launch(|| {
		pause();
		during.store(nsnow(), Ordering::Relaxed);
	}, u64::max_value()).unwrap()).collect();

	let mut into = 0;
	let mut outof = 0;
	let mut index = 0;
	lo.iter(|| {
		assert!(index < lingers.len(), "LIBSETS tunable set too low!");

		let before = nsnow();
		resume(&mut lingers[index], u64::max_value()).unwrap();

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

	for linger in &mut lingers[index..] {
		resume(linger, u64::max_value()).unwrap();
	}
}

fn renew(lo: &mut Bencher) {
	use inger::launch;

	let lingers: Vec<_> = (0..LIBSETS).map(|_| launch(pause, u64::max_value()).unwrap()).collect();
	let mut lingers = lingers.into_iter();
	lo.iter(||
		drop(lingers.next())
	);
}

benchmark_main! {
	bench
}
