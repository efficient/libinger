use dlfcn::Handle;
use dlfcn::dlsym;
use libc::c_int;
use libc::c_void;
use libc::size_t;

struct Funs {
	malloc: unsafe extern "C" fn(size_t) -> *mut c_void,
	calloc: unsafe extern "C" fn(size_t, size_t) -> *mut c_void,
	realloc: unsafe extern "C" fn(*mut c_void, size_t) -> *mut c_void,
	posix_memalign: unsafe extern "C" fn(*mut *mut c_void, size_t, size_t) -> c_int,
	free: unsafe extern "C" fn(*mut c_void),
}

thread_local! {
	static FUNS: Funs = Funs {
		malloc: dlsym(Handle::next(), b"malloc\0").unwrap().unwrap(),
		calloc: dlsym(Handle::next(), b"calloc\0").unwrap().unwrap(),
		realloc: dlsym(Handle::next(), b"realloc\0").unwrap().unwrap(),
		posix_memalign: dlsym(Handle::next(), b"posix_memalign\0").unwrap().unwrap(),
		free: dlsym(Handle::next(), b"free\0").unwrap().unwrap(),
	};
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: size_t) -> *mut c_void {
	shallow_call(|| FUNS.with(|funs| funs.malloc)(size))
}

#[no_mangle]
pub unsafe extern "C" fn calloc(nobj: size_t, size: size_t) -> *mut c_void {
	shallow_call(|| FUNS.with(|funs| funs.calloc)(nobj, size))
}

#[no_mangle]
pub unsafe extern "C" fn realloc(addr: *mut c_void, size: size_t) -> *mut c_void {
	shallow_call(|| FUNS.with(|funs| funs.realloc)(addr, size))
}

#[no_mangle]
pub unsafe extern "C" fn posix_memalign(addr: *mut *mut c_void, align: size_t, size: size_t) -> c_int {
	shallow_call(|| FUNS.with(|funs| funs.posix_memalign)(addr, align, size))
}

#[no_mangle]
pub unsafe extern "C" fn free(addr: *mut c_void) {
	shallow_call(|| FUNS.with(|funs| funs.free)(addr));
}

/// "Thunk," guarding against mutual recursion.
///
/// We have a bootstrapping crisis: `dlsym()` allocates memory during the initialization of `FUNS`!
/// Clearly we can't permit the nested call because it, too, would find `FUNS` uninitialized and
/// incite infinite recursion.  Fortunately, glibc's `dlsym()` implementation gracefully falls back
/// to using a static buffer if its allocation request fails (see `dlerror.c:_dlerror_run()`),
/// behavior that we can exploit by returning `NULL` upon detecting (potentially) mutual recursion.
/// The only sticky situation to handle is threads that simultaneously attempt to allocate memory
/// *for the first time*.
fn shallow_call<T: Optional, F: Fn() -> T>(thunk: F) -> T {
	use std::cell::Cell;

	thread_local! {
		static RECURSING: Cell<bool> = Cell::new(false);
	}

	if RECURSING.with(|recursing| recursing.replace(true)) {
		T::none()
	} else {
		let res = thunk();
		RECURSING.with(|recursing| recursing.set(false));
		res
	}
}

trait Optional {
	fn none() -> Self;
}

impl Optional for () {
	fn none() -> Self {
		()
	}
}

impl Optional for i32 {
	fn none() -> Self {
		use libc::ENOMEM;

		ENOMEM
	}
}

impl<T> Optional for *mut T {
	fn none() -> Self {
		use std::ptr::null_mut;

		null_mut()
	}
}
