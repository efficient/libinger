use linger::Linger as Lingerer;
use std::ffi::c_void;
use std::process::abort;
use std::thread::Result;

#[repr(C)]
pub struct Linger {
	is_complete: bool,
	continuation: Lingerer<(), dyn FnMut(*mut Option<Result<()>>) + Send>,
}

#[no_mangle]
extern fn launch(fun: unsafe extern fn(*mut c_void), us: u64, args: *mut c_void) -> Linger {
	use force::AssertSend;
	use linger::launch;

	let args = unsafe {
		AssertSend::new(args)
	};
	let timed = launch(move || unsafe {
		fun(*args)
	}, us);
	if let Ok(timed) = timed {
		Linger {
			is_complete: timed.is_completion(),
			continuation: timed.erase(),
		}
	} else {
		abort()
	}
}

#[no_mangle]
extern fn resume(timed: Option<&mut Linger>, us: u64) {
	use linger::resume;

	if let Some(timed) = timed {
		if resume(&mut timed.continuation, us).is_err() {
			abort();
		}
		timed.is_complete = timed.continuation.is_completion();
	} else {
		abort();
	}
}

#[no_mangle]
extern fn cancel(timed: Option<&mut Linger>) {
	if let Some(timed) = timed {
		if timed.continuation.is_continuation() {
			timed.continuation = Lingerer::Completion(());
			timed.is_complete = true;
		}
	} else {
		abort();
	}
}

#[no_mangle]
extern fn pause() {
	use linger::pause;

	pause();
}
