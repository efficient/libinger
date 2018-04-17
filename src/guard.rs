use signal::Operation;
use signal::Set;
use signal::Signal;
use signal::Sigset;
use signal::sigprocmask;
use std::io::Result;

pub struct PreemptGuard {
	blocking: bool,
}

impl PreemptGuard {
	pub fn block() -> Result<Self> {
		let mut masked = Sigset::empty();
		let mut mask = Sigset::empty();
		mask.add(Signal::Alarm);
		sigprocmask(Operation::Block, &mask, Some(&mut masked))?;

		Ok(Self {
			// In the case of all but the root thread, we mask the signal by interposing
			// our own pthread_create().  However, if code uses a PreemptGuard from the
			// main thread (e.g. by calling one of the handled A[CS]-unsafe library
			// functions, asserting the signal would kill us if the client code has
			// never launch()'d a timed task.  Detect this case and instead mask the
			// signal indefinitely.
			blocking: ! (masked.has(Signal::Alarm) || is_initial_thread()),
		})
	}

	pub fn unblock() -> Result<()> {
		use pthread::pthread_kill;
		use pthread::pthread_self;
		use std::thread::panicking;

		let mut mask = Sigset::empty();
		mask.add(Signal::Alarm);
		sigprocmask(Operation::Unblock, &mask, None)?;
		if ! panicking() {
			pthread_kill(pthread_self(), Signal::Alarm)?;
		}

		Ok(())
	}
}

impl Drop for PreemptGuard {
	fn drop(&mut self) {
		if self.blocking {
			Self::unblock().unwrap();
		}
	}
}

fn is_initial_thread() -> bool {
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	static INIT: Once = ONCE_INIT;
	let mut res = false;

	INIT.call_once(|| res = true);

	res
}
