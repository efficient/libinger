#![cfg_attr(bench, feature(test))]

extern crate inger;
extern crate libc;
extern crate signal;
#[cfg(bench)]
extern crate test;

mod lock;

use inger::Linger;
use inger::launch;
use inger::nsnow;
use inger::resume;
use lock::sigalrm_lock;
#[cfg(bench)]
use test::Bencher;

#[test]
fn launch_completion() {
	let mut lock = sigalrm_lock();
	lock.preserve();
	assert!(launch(|| (), 1_000).unwrap().is_completion());
	drop(lock);
}

#[test]
fn launch_continuation() {
	let mut lock = sigalrm_lock();
	lock.preserve();
	assert!(launch(|| timeout(1_000_000), 10).unwrap().is_continuation());
	drop(lock);
}

#[should_panic(expected = "PASS")]
#[test]
fn launch_panic() {
	let mut lock = sigalrm_lock();
	lock.preserve();
	drop(launch(|| panic!("PASS"), 1_000));
	// Lock becomes poisoned.
}

#[should_panic(expected = "PASS")]
#[test]
fn launch_panic_outer() {
	let mut lock = sigalrm_lock();
	lock.preserve();
	drop(launch(|| {
		drop(launch(|| (), 1_000));
		panic!("PASS");
	}, 1_000));
	// Lock becomes poisoned.
}

#[should_panic(expected = "PASS")]
#[test]
fn launch_panic_inner() {
	let mut lock = sigalrm_lock();
	lock.preserve();
	drop(launch(|| drop(launch(|| panic!("PASS"), 1_000)), 1_000));
	// Lock becomes poisoned.
}

#[test]
fn launch_completions() {
	let mut lock = sigalrm_lock();
	lock.preserve();
	assert!(launch(|| assert!(launch(|| (), 1_000).unwrap().is_completion()), 1_000).unwrap().is_completion());
	drop(lock);
}

#[test]
fn launch_continuations() {
	let mut lock = sigalrm_lock();
	lock.preserve();
	assert!(launch(|| {
		assert!(launch(|| timeout(1_000_000), 10).unwrap().is_continuation());
		timeout(1_000_000);
	}, 1_000).unwrap().is_continuation());
	drop(lock);
}

#[test]
fn resume_completion() {
	let mut lock = sigalrm_lock();
	lock.preserve();

	let mut cont = launch(|| timeout(1_000_000), 10).unwrap();
	assert!(cont.is_continuation(), "completion instead of continuation");
	assert!(resume(&mut cont, 10_000_000).unwrap().is_completion());
	drop(lock);
}

#[test]
fn resume_completion_drop() {
	let mut lock = sigalrm_lock();
	lock.preserve();

	let mut cont = launch(|| timeout(1_000_000), 100).unwrap();
	assert!(cont.is_continuation(), "completion instead of continuation");
	assert!(resume(&mut cont, 10_000).unwrap().is_continuation());
	drop(lock);
}

#[test]
fn resume_completion_repeat() {
	let mut lock = sigalrm_lock();
	lock.preserve();

	let mut cont = launch(|| timeout(1_000_000), 10).unwrap();
	assert!(cont.is_continuation(), "launch(): returned completion instead of continuation");
	resume(&mut cont, 10).unwrap();
	assert!(cont.is_continuation(), "resume(): returned completion instead of continuation");
	assert!(resume(&mut cont, 10_000_000).unwrap().is_completion());
	drop(lock);
}

#[test]
fn abuse_preemption() {
	for _ in 0..10_000 {
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
