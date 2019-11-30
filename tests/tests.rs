extern crate libc;
extern crate timetravel;

use std::cell::Cell;
use timetravel::stable::StableMutAddr;
use timetravel::Context;
use timetravel::getcontext;
use timetravel::makecontext;
use timetravel::restorecontext;
use timetravel::setcontext;
use timetravel::sigsetcontext;

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
			let thing: *const _ = context.borrow_mut().get_or_insert(thing);
			panic!(setcontext(thing));
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
				let thing: *const _ = inner.borrow_mut().get_or_insert(thing);
				panic!(setcontext(thing));
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

fn swap(afterparty: Option<fn()>) -> Context<Box<[u8]>> {
	use libc::SIGSTKSZ;
	use std::cell::RefCell;

	thread_local! {
		static CONTEXT: RefCell<Option<Context<Box<[u8]>>>> = RefCell::new(None);
		static AFTERPARTY: Cell<fn()> = Cell::new(default);
	}

	fn default() { unreachable!(); }
	if let Some(fun) = afterparty {
		AFTERPARTY.with(|afterparty| afterparty.set(fun));
	} else {
		AFTERPARTY.with(|afterparty| afterparty.set(default));
	}

	let stack: Box<[_]> = Box::new([0u8; 2 * SIGSTKSZ]);
	makecontext(
		stack,
		|thing| {
			let thing = CONTEXT.with(|context| -> *const _ {
				context.borrow_mut().get_or_insert(thing)
			});
			panic!(setcontext(thing));
		},
		|| {
			let checkpoint = CONTEXT.with(|context| -> *mut _ {
				context.borrow_mut().as_mut().unwrap()
			});
			swap_helper(checkpoint);
			AFTERPARTY.with(|afterparty| afterparty.get())();
		},
	).unwrap();

	CONTEXT.with(|context| context.borrow_mut().take()).unwrap()
}

#[test]
fn swap_unreached() {
	swap(None);
}

#[test]
fn swap_expired() {
	assert!(sigsetcontext(&mut swap(None)).is_none());
}

#[should_panic(expected = "true")]
#[test]
fn swap_incorrect() {
	let mut none = false;
	restorecontext(swap(None), |checkpoint| none = setcontext(&checkpoint).is_none()).unwrap();
	panic!(format!("{}", none));
}

#[should_panic(expected = "true")]
#[test]
fn swap_reached() {
	thread_local! {
		static REACHED: Cell<bool> = Cell::new(false);
	}

	restorecontext(
		swap(Some(|| REACHED.with(|reached| reached.set(true)))),
		|mut checkpoint| panic!(sigsetcontext(&mut checkpoint))
	).unwrap();
	panic!(format!("{}", REACHED.with(|reached| reached.get())));
}

#[should_panic(expected = "true")]
#[test]
fn swap_repeated() {
	use std::cell::RefCell;

	thread_local! {
		static CHECKPOINT: RefCell<Option<Context<Box<[u8]>>>> = RefCell::new(None);
		static REACHED: Cell<bool> = Cell::new(false);
	}

	let checkpoint = swap(Some(|| {
		swap_helper(CHECKPOINT.with(|checkpoint| -> *mut _ {
			checkpoint.borrow_mut().as_mut().unwrap()
		}));
		REACHED.with(|reached| reached.set(true));
	}));

	restorecontext(
		checkpoint,
		|checkpoint| {
			let checkpoint = CHECKPOINT.with(|point| -> *mut _ {
				point.borrow_mut().get_or_insert(checkpoint)
			});
			panic!(sigsetcontext(checkpoint));
		},
	).unwrap();
	assert!(! REACHED.with(|reached| reached.get()));

	restorecontext(
		CHECKPOINT.with(|checkpoint| checkpoint.borrow_mut().take()).unwrap(),
		|mut checkpoint| panic!(sigsetcontext(&mut checkpoint)),
	).unwrap();
	panic!(format!("{}", REACHED.with(|reached| reached.get())));
}

fn swap_helper<T: StableMutAddr<Target = [u8]>>(context: *mut Context<T>) {
	use libc::SA_SIGINFO;
	use libc::SIGUSR1;
	use libc::pthread_kill;
	use libc::pthread_self;
	use libc::sigaction;
	use libc::siginfo_t;
	use std::io::Error;
	use std::mem::transmute;
	use std::mem::zeroed;
	use std::os::raw::c_int;
	use std::ptr::null_mut;
	use timetravel::HandlerContext;
	use timetravel::Swap;

	thread_local! {
		static CHECKPOINT: Cell<Option<&'static mut dyn Swap<Other = HandlerContext>>> =
			Cell::new(None);
	}

	extern "C" fn handler(
		_: c_int,
		_: Option<&mut siginfo_t>,
		context: Option<&mut HandlerContext>,
	) {
		let checkpoint = CHECKPOINT.with(|checkpoint| checkpoint.take()).unwrap();
		checkpoint.swap(context.unwrap());
	}

	let config = sigaction {
		sa_flags: SA_SIGINFO,
		sa_sigaction: handler as _,
		sa_restorer: None,
		sa_mask: unsafe {
			zeroed()
		},
	};
	if unsafe {
		sigaction(SIGUSR1, &config, null_mut())
	} != 0 {
		panic!(Error::last_os_error());
	}

	let context = unsafe {
		&mut *context
	};
	let context: &mut dyn Swap<Other = HandlerContext> = context;
	let context: Option<&'static mut (dyn Swap<Other = HandlerContext> + 'static)> = Some(unsafe {
		transmute(context)
	});
	CHECKPOINT.with(|checkpoint| checkpoint.set(context));

	unsafe {
		pthread_kill(pthread_self(), SIGUSR1);
	}
}
