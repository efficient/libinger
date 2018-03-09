use dlfcn::Handle;
use dlfcn::dlsym;
use libc::c_int;
use libc::c_void;
use libc::pthread_attr_t;
use libc::pthread_t;
use signal::Operation;
use signal::Set;
use signal::Signal;
use signal::Sigset;
use signal::sigprocmask;
use std::io::Error;
use std::io::Result;

pub struct PThread (pthread_t);

pub fn pthread_kill(thread: PThread, signal: Signal) -> Result<()> {
	use libc::pthread_kill;

	let code = unsafe {
		pthread_kill(thread.0, signal as c_int)
	};
	if code == 0 {
		Ok(())
	} else {
		Err(Error::from_raw_os_error(code))
	}
}

pub fn pthread_self() -> PThread {
	use libc::pthread_self;

	PThread (unsafe {
		pthread_self()
	})
}

#[no_mangle]
pub unsafe extern "C" fn pthread_create(thread: *mut pthread_t, attr: *const pthread_attr_t, routine: extern "C" fn(*mut c_void) -> *mut c_void, arg: *mut c_void) -> c_int {
	let pthread_create: unsafe extern "C" fn(*mut pthread_t, *const pthread_attr_t, extern "C" fn(*mut c_void) -> *mut c_void, *mut c_void) -> i32 =
		dlsym(Handle::next(), b"pthread_create\0").unwrap().unwrap();

	let mut mask = Sigset::empty();
	mask.add(Signal::Alarm);
	sigprocmask(Operation::Block, &mask, None).unwrap();

	pthread_create(thread, attr, routine, arg)
}
