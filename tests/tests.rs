#![cfg_attr(not(test), allow(dead_code))]

extern crate ucontext;

fn main() {
	getcontext_donothing();
	getcontext_setcontext();
	getcontext_succeedatnothing();
	getcontext_nested();
}

#[cfg_attr(test, should_panic(expected = "done"))]
#[cfg_attr(test, test)]
fn getcontext_donothing() {
	use ucontext::getcontext;

	let mut reached = false;
	getcontext(|_| reached = true, || unreachable!()).unwrap();
	assert!(reached);
	panic!("done");
}

#[cfg_attr(test, should_panic(expected = "done"))]
#[cfg_attr(test, test)]
fn getcontext_setcontext() {
	use ucontext::getcontext;
	use ucontext::setcontext;

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
	use ucontext::getcontext;
	use ucontext::setcontext;

	let invalid = getcontext(|context| context, || unreachable!()).unwrap();
	assert!(setcontext(invalid).is_none());
	panic!("done");
}

#[cfg_attr(test, should_panic(expected = "done"))]
#[cfg_attr(test, test)]
fn getcontext_nested() {
	use std::cell::Cell;
	use ucontext::getcontext;
	use ucontext::setcontext;

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
