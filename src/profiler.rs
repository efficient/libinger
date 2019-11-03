use linger::nsnow;
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
