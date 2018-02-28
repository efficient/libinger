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

struct StartRoutine {
	routine: extern "C" fn(*mut c_void) -> *mut c_void,
	arg: *mut c_void,
}

#[no_mangle]
pub unsafe extern "C" fn pthread_create(thread: *mut pthread_t, attr: *const pthread_attr_t, routine: extern "C" fn(*mut c_void) -> *mut c_void, arg: *mut c_void) -> c_int {
	let pthread_create: unsafe extern "C" fn(*mut pthread_t, *const pthread_attr_t, extern "C" fn(*mut c_void) -> *mut c_void, *mut c_void) -> i32 =
		dlsym(Handle::next(), b"pthread_create\0").unwrap().unwrap();
	let arg = Box::new(StartRoutine {
		routine: routine,
		arg: arg,
	});

	pthread_create(thread, attr, pthread_creator, Box::into_raw(arg) as *mut c_void)
}

extern "C" fn pthread_creator(payload: *mut c_void) -> *mut c_void {
	let payload = unsafe {
		Box::from_raw(payload as *mut StartRoutine)
	};

	let mut mask = Sigset::empty();
	mask.add(Signal::Alarm);
	sigprocmask(Operation::Block, &mask, None).unwrap();

	(payload.routine)(payload.arg)
}
