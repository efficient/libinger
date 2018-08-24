extern crate libc;
extern crate timetravel;

use timetravel::getcontext;
use timetravel::makecontext;
use timetravel::restorecontext;
use timetravel::setcontext;

#[test]
fn get_expired() {
	let context = getcontext(|context| context, || unreachable!()).unwrap();
	assert!(setcontext(&context).is_none());
}

#[test]
fn make_expired() {
	use libc::MINSIGSTKSZ;

	let mut stack = [0u8; MINSIGSTKSZ];
	let mut context = None;
	makecontext(&mut stack[..], |thing| context = Some(thing), || unreachable!()).unwrap();
	assert!(setcontext(context.as_ref().unwrap()).is_none());
}

#[test]
fn restore_expired() {
	use libc::MINSIGSTKSZ;

	let stack: Box<[_]> = Box::new([0u8; MINSIGSTKSZ]);
	let mut context = None;
	makecontext(stack, |thing| context = Some(thing), || unreachable!()).unwrap();
	restorecontext(context.take().unwrap(), |thing| context = Some(thing)).unwrap();
	assert!(setcontext(context.as_ref().unwrap()).is_none());
}

#[should_panic(expected = "true")]
#[test]
fn get_reached() {
	let mut reached = false;
	getcontext(|context| panic!(setcontext(&context)), || reached = true).unwrap();
	panic!(format!("{}", reached));
}

#[should_panic(expected = "true")]
#[test]
fn make_reached() {
	use libc::MINSIGSTKSZ;
	use std::cell::Cell;

	thread_local! {
		static REACHED: Cell<bool> = Cell::new(false);
	}

	let mut stack = [0u8; MINSIGSTKSZ];
	makecontext(
		&mut stack[..],
		|gate| panic!(setcontext(&gate)),
		|| REACHED.with(|reached| reached.set(true)),
	).unwrap();
	REACHED.with(|reached| panic!(format!("{}", reached.get())));
}

#[should_panic(expected = "true")]
#[test]
fn restore_reached() {
	use libc::MINSIGSTKSZ;
	use std::cell::Cell;

	thread_local! {
		static REACHED: Cell<bool> = Cell::new(false);
	}

	let stack: Box<[_]> = Box::new([0u8; MINSIGSTKSZ]);
	let mut gate = None;
	makecontext(
		stack,
		|thing| gate = Some(thing),
		|| REACHED.with(|reached| reached.set(true)),
	).unwrap();
	restorecontext(gate.take().unwrap(), |gate| panic!(setcontext(&gate))).unwrap();
	REACHED.with(|reached| panic!(format!("{}", reached.get())));
}

#[should_panic(expected = "true")]
#[test]
fn get_repeated() {
	use std::cell::RefCell;

	let mut reached = true;
	let context = RefCell::new(None);
	getcontext(
		|thing| {
			context.replace(Some(thing));
			panic!(setcontext(context.borrow().as_ref().unwrap()));
		},
		|| if context.try_borrow().is_ok() {
			panic!(setcontext(context.borrow_mut().as_ref().unwrap()));
		} else {
			reached = true;
		},
	).unwrap();
	panic!(format!("{}", reached));
}

#[should_panic(expected = "true")]
#[test]
fn get_nested() {
	use std::cell::RefCell;

	let mut reached = false;
	let inner = RefCell::new(None);
	getcontext(
		|outer| panic!(getcontext(
			|thing| {
				inner.replace(Some(thing));
				panic!(setcontext(inner.borrow().as_ref().unwrap()));
			},
			|| {
				panic!(setcontext(&outer));
			},
		)),
		|| {
			reached = true;
			assert!(setcontext(inner.borrow().as_ref().unwrap()).is_none());
		},
	).unwrap();
	panic!(format!("{}", reached));
}
