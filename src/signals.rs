use signal::Signal;
use reusable::SyncResult;

static NOTIFICATION_SIGNALS: [Signal; 16] = [
	Signal::Alarm,
	Signal::VirtualAlarm,
	Signal::ProfilingTimer,
	Signal::ProcessorLimit,
	Signal::FilesystemLimit,
	Signal::TerminalInput,
	Signal::TerminalOutput,
	Signal::PowerFailure,
	Signal::User1,
	Signal::User2,

	// A stretch...
	Signal::UrgentSocket,
	Signal::Pollable,
	Signal::Syscall,
	Signal::FloatingPoint,
	Signal::Hangup,
	Signal::Child,
];

pub fn assign_signal() -> SyncResult<'static, Signal> {
	use compile_assert::assert_sync;
	use reusable::SyncPool;
	use std::convert::TryInto;
	use std::sync::atomic::AtomicUsize;
	use std::sync::atomic::Ordering;
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	static mut SIGNALS: Option<SyncPool<Signal, Box<dyn Fn() -> Option<Signal> + Sync>>> = None;
	static INIT: Once = ONCE_INIT;
	INIT.call_once(|| unsafe {
		let free = AtomicUsize::new(0);
		SIGNALS.replace(SyncPool::new(Box::new(move ||
			NOTIFICATION_SIGNALS.get(free.fetch_add(1, Ordering::Relaxed)).copied()
		)));
	});

	let signals = unsafe {
		SIGNALS.as_ref()
	}.unwrap();
	assert_sync(&signals);
	signals.try_into()
}
