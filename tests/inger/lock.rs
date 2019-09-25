use std::sync::MutexGuard;

pub fn exclusive<T>(fun: fn() -> T) {
	let lock = lock();
	fun();
	drop(lock);
}

fn lock() -> MutexGuard<'static, ()> {
	use std::sync::ONCE_INIT;
	use std::sync::Mutex;
	use std::sync::Once;

	static INIT: Once = ONCE_INIT;
	static mut LOCK: Option<Mutex<()>> = None;

	INIT.call_once(|| unsafe {
		LOCK.replace(Mutex::new(()));
	});

	// The lock might be poisened because a previous test failed. This is safe to ignore
	// because we should no longer have a race (since the other test's thread is now
	// dead) and we don't need to fail the current test as well.
	unsafe {
		LOCK.as_ref()
	}.unwrap().lock().unwrap_or_else(|poison| poison.into_inner())
}
