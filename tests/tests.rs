#![cfg_attr(not(test), allow(dead_code))]

extern crate ucontext;

use ucontext::getcontext;
use ucontext::makecontext;
use ucontext::setcontext;

fn main() {
	getcontext_donothing();
	getcontext_setcontext();
	getcontext_succeedatnothing();
	//getcontext_nested();
	makecontext_setcontext();
}

#[cfg_attr(test, should_panic(expected = "done"))]
#[cfg_attr(test, test)]
fn getcontext_donothing() {
	let mut reached = false;
	getcontext(|_| reached = true, || unreachable!()).unwrap();
	assert!(reached);
	panic!("done");
}

#[cfg_attr(test, should_panic(expected = "done"))]
#[cfg_attr(test, test)]
fn getcontext_setcontext() {
	let mut reached = false;
	getcontext(
		|context| {
			setcontext(context);
			unreachable!();
		},
		|| reached = true,
	).unwrap();
	assert!(reached);
	panic!("done");
}

#[cfg_attr(test, should_panic(expected = "done"))]
#[cfg_attr(test, test)]
fn getcontext_succeedatnothing() {
	let invalid = getcontext(|context| context, || unreachable!()).unwrap();
	assert!(setcontext(invalid).is_none());
	panic!("done");
}

#[cfg_attr(test, ignore)]
#[cfg_attr(test, should_panic(expected = "done"))]
#[cfg_attr(test, test)]
fn getcontext_nested() {
	use std::cell::Cell;

	let mut reached = true;
	let context = Cell::new(None);
	getcontext(
		|outer| getcontext(
			|inner| {
				context.set(Some(inner));
				setcontext(outer);
				unreachable!();
			},
			|| reached = true,
		).unwrap(),
		|| {
			setcontext(context.take().unwrap());
			unreachable!();
		},
	).unwrap();
	assert!(reached);
	panic!("done");
}

#[cfg_attr(test, should_panic(expected = "done"))]
#[cfg_attr(test, test)]
fn makecontext_setcontext() {
	use std::cell::Cell;
	use ucontext::MINSIGSTKSZ;

	thread_local! {
		static REACHED: Cell<bool> = Cell::new(false);
	}

	extern "C" fn call() {
		REACHED.with(|reached| reached.set(true));
	}

	let mut reached = false;
	getcontext(
		|mut successor| {
			let mut stack = [0u8; MINSIGSTKSZ];
			let predecessor = makecontext(call, &mut stack, Some(&mut successor)).unwrap();
			setcontext(predecessor);
			unreachable!();
		},
		|| reached = true,
	).unwrap();
	assert!(REACHED.with(|reached| reached.get()));
	assert!(reached);
	panic!("done");
}
