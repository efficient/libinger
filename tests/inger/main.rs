#![cfg_attr(bench, feature(test))]

extern crate inger;
#[cfg(bench)]
extern crate test;

mod lock;

use inger::launch;
use inger::nsnow;
use inger::resume;
use lock::exclusive;
#[cfg(bench)]
use test::Bencher;

#[test]
fn launch_completion() {
	exclusive(||
		assert!(launch(|| (), 1_000).unwrap().is_completion())
	);
}

#[test]
fn launch_continuation() {
	exclusive(||
		assert!(launch(|| timeout(1_000_000), 10).unwrap().is_continuation())
	);
}

#[test]
fn launch_union() {
	exclusive(||
		launch(|| -> Result<bool, Box<()>> { Ok(false) }, 1_000).unwrap()
	);
}

#[should_panic(expected = "PASS")]
#[test]
fn launch_panic() {
	exclusive(||
		drop(launch(|| panic!("PASS"), 1_000))
		// Lock becomes poisoned.
	);
}

#[ignore]
#[should_panic(expected = "PASS")]
#[test]
fn launch_panic_outer() {
	exclusive(||
		drop(launch(|| {
			drop(launch(|| (), 1_000));
			panic!("PASS");
		}, 1_000))
		// Lock becomes poisoned.
	);
}

#[ignore]
#[should_panic(expected = "PASS")]
#[test]
fn launch_panic_inner() {
	exclusive(||
		drop(launch(|| drop(launch(|| panic!("PASS"), 1_000)), 1_000))
		// Lock becomes poisoned.
	);
}

#[ignore]
#[test]
fn launch_completions() {
	exclusive(||
		assert!(launch(|| assert!(launch(|| (), 1_000).unwrap().is_completion()), 1_000).unwrap().is_completion())
	);
}

#[ignore]
#[test]
fn launch_continuations() {
	exclusive(|| {
		assert!(launch(|| {
			assert!(launch(|| timeout(1_000_000), 10).unwrap().is_continuation());
			timeout(1_000_000);
		}, 1_000).unwrap().is_continuation());
	});
}

#[test]
fn resume_completion() {
	exclusive(|| {
		let mut cont = launch(|| timeout(1_000_000), 10).unwrap();
		assert!(cont.is_continuation(), "completion instead of continuation");
		assert!(resume(&mut cont, 10_000_000).unwrap().is_completion());
	});
}

#[test]
fn resume_completion_drop() {
	exclusive(|| {
		let mut cont = launch(|| timeout(1_000_000), 100).unwrap();
		assert!(cont.is_continuation(), "completion instead of continuation");
		assert!(resume(&mut cont, 10_000).unwrap().is_continuation());
	});
}

#[test]
fn resume_completion_repeat() {
	exclusive(|| {
		let mut cont = launch(|| timeout(1_000_000), 10).unwrap();
		assert!(cont.is_continuation(), "launch(): returned completion instead of continuation");
		resume(&mut cont, 10).unwrap();
		assert!(cont.is_continuation(), "resume(): returned completion instead of continuation");
		assert!(resume(&mut cont, 10_000_000).unwrap().is_completion());
	});
}

#[test]
fn setup_only() {
	use std::sync::atomic::AtomicBool;
	use std::sync::atomic::Ordering;

	exclusive(|| {
		let run = AtomicBool::new(false);
		let mut prep = launch(|| run.store(true, Ordering::Relaxed), 0).unwrap();
		assert!(! run.load(Ordering::Relaxed));
		resume(&mut prep, 1_000).unwrap();
		assert!(run.load(Ordering::Relaxed));
	});
}

#[should_panic(expected = "launch(): too many active timed functions: None")]
#[test]
fn launch_toomany() {
	exclusive(|| {
		let _thing_one = launch(|| timeout(1_000_000), 0).unwrap();
		let _thing_two = launch(|| timeout(1_000_000), 0).unwrap();
		let _thing_three = launch(|| timeout(1_000_000), 0).unwrap();
		// Lock becomes poisoned.
	});
}

#[test]
fn launch_toomany_reinit() {
	exclusive(|| {
		let thing_one = launch(|| timeout(1_000_000), 0).unwrap();
		let _thing_two = launch(|| timeout(1_000_000), 0).unwrap();
		drop(thing_one);
		let _thing_three = launch(|| timeout(1_000_000), 0).unwrap();
	});
}

#[test]
fn abuse_preemption() {
	for _ in 0..25 {
		launch_continuation();
	}
}

fn timeout(mut useconds: u64) {
	useconds *= 1_000;

	let mut elapsed = 0;
	let mut last = nsnow();
	while elapsed < useconds {
		let mut this = nsnow();
		while this < last || this - last > 1_000 {
			last = this;
			this = nsnow();
		}
		elapsed += this - last;
		last = this;
	}
}

#[bench]
#[cfg(bench)]
fn timeout_10(lo: &mut Bencher) {
	lo.iter(|| timeout(10));
}

#[bench]
#[cfg(bench)]
fn timeout_100(lo: &mut Bencher) {
	lo.iter(|| timeout(100));
}

#[bench]
#[cfg(bench)]
fn timeout_1000(lo: &mut Bencher) {
	lo.iter(|| timeout(1_000));
}
