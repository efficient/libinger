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
			blocking: ! masked.has(Signal::Alarm),
		})
	}

	pub fn unblock() -> Result<()> {
		let mut mask = Sigset::empty();
		mask.add(Signal::Alarm);
		sigprocmask(Operation::Unblock, &mask, None)
	}
}

impl Drop for PreemptGuard {
	fn drop(&mut self) {
		if self.blocking {
			Self::unblock().unwrap();
		}
	}
}
