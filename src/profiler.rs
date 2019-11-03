use libc::ucontext_t;
use linger::nsnow;
use signal::Set;
use signal::Signal;
use signal::Sigset;
use signal::siginfo_t;
use std::cell::RefCell;
use std::collections::VecDeque;

pub fn with_profiler<T>(fun: impl Fn(&mut Profiler) -> T) -> T {
	thread_local! {
		static PROFILER: RefCell<Option<Profiler>> = RefCell::default();
	}
	PROFILER.with(|profiler|
		fun(profiler.borrow_mut().get_or_insert_with(Profiler::default))
	)
}

#[derive(Default)]
pub struct Profiler {
	past: VecDeque<u64>,
	present: u64,
}

impl Profiler {
	pub fn begin(&mut self) {
		use signal::Action;
		use signal::Sigaction;
		use signal::sigaction;
		use std::sync::Once;

		static ONCE: Once = Once::new();
		ONCE.call_once(||
			drop(sigaction(Signal::Interrupt, &mut Sigaction::new(interrupt, Sigset::empty(), 0), None))
		);
		self.present = nsnow();
	}

	pub fn end(&mut self) -> bool {
		if self.present != 0 {
			self.past.push_back(nsnow() - self.present);
			self.present = 0;
			true
		} else {
			false
		}
	}
}

impl Drop for Profiler {
	fn drop(&mut self) {
		let len: f64 = self.past.len() as _;
		let sum: u64 = self.past.iter().sum();
		let sum: f64 = sum as _;
		println!("Profiler ave. = {} us", sum / len / 1_000.0);
	}
}

extern fn interrupt(_: Signal, _: Option<&siginfo_t>, _: Option<&mut ucontext_t>) {
	use signal::Operation;
	use signal::pthread_sigmask;
	use std::os::raw::c_int;
	use std::process::abort;
	use std::thread::current;
	extern {
		fn exit(_: c_int);
	}
	if current().name().unwrap_or_else(|| abort()) != "main" {
		unsafe {
			exit(100);
		}
	} else {
		let mut sig = Sigset::empty();
		sig.add(Signal::Interrupt);
		drop(pthread_sigmask(Operation::Block, &sig, None))
	}
}
