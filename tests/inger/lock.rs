use signal::Sigaction;
use std::ops::Deref;
use std::sync::MutexGuard;

pub fn exclusive<T>(fun: fn() -> T) {
	let mut lock = sigalrm_lock();
	lock.preserve();
	fun();
	drop(lock);
}

fn sigalrm_lock() -> Restorer<Sigaction, Box<Fn() -> Sigaction>, Box<Fn(&Sigaction)>, MutexGuard<'static, (Box<Fn() -> Sigaction>, Box<Fn(&Sigaction)>)>> {
	use libc::ucontext_t;
	use signal::Action;
	use signal::Set;
	use signal::Signal;
	use signal::Sigset;
	use signal::sigaction;
	use signal::siginfo_t;
	use std::sync::ONCE_INIT;
	use std::sync::Once;
	use std::sync::Mutex;

	static INIT: Once = ONCE_INIT;
	static mut LOCK: Option<Mutex<(Box<Fn() -> Sigaction>, Box<Fn(&Sigaction)>)>> = None;

	INIT.call_once(|| {
		let save = || {
			extern "C" fn dummy(_: Signal, _: Option<&siginfo_t>, _: Option<&mut ucontext_t>) {}
			let mut res = Sigaction::new(dummy, Sigset::empty(), 0);
			sigaction(Signal::Alarm, &(), Some(&mut res)).unwrap();
			res
		};
		let restore = |it: &Sigaction| sigaction(Signal::Alarm, it, None).unwrap();

		unsafe {
			LOCK = Some(Mutex::new((Box::new(save), Box::new(restore))))
		}
	});

	// The lock might be poisened because a previous test failed. This is safe to ignore
	// because we should no longer have a race (since the other test's thread is now
	// dead) and we don't need to fail the current test as well.
	Restorer::new(unsafe {
		LOCK.as_ref().unwrap()
	}.lock().unwrap_or_else(|poison| poison.into_inner()))
}

pub struct Restorer<T, A, B: Deref<Target = Fn(&T)>, F: Deref<Target = (A, B)>> {
	done: T,
	fun: F,
	run: bool,
}

impl<T, A: Deref<Target = Fn() -> T>, B: Deref<Target = Fn(&T)>, F: Deref<Target = (A, B)>> Restorer<T, A, B, F> {
	fn new(funs: F) -> Self {
		Self {
			done: funs.0(),
			fun: funs,
			run: true,
		}
	}

	pub fn preserve(&mut self) {
		self.run = false;
	}
}

impl<T, A, B: Deref<Target = Fn(&T)>, F: Deref<Target = (A, B)>> Drop for Restorer<T, A, B, F> {
	fn drop(&mut self) {
		if self.run {
			(self.fun.1)(&self.done);
		}
	}
}
