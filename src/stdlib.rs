use libc::c_int;
use libc::c_void;
use libc::size_t;
use self::funs::shallow_call;

#[no_mangle]
pub unsafe extern "C" fn malloc(size: size_t) -> *mut c_void {
	shallow_call(|funs| (funs.malloc)(size))
}

#[no_mangle]
pub unsafe extern "C" fn calloc(nobj: size_t, size: size_t) -> *mut c_void {
	shallow_call(|funs| (funs.calloc)(nobj, size))
}

#[no_mangle]
pub unsafe extern "C" fn realloc(addr: *mut c_void, size: size_t) -> *mut c_void {
	shallow_call(|funs| (funs.realloc)(addr, size))
}

#[no_mangle]
pub unsafe extern "C" fn posix_memalign(addr: *mut *mut c_void, align: size_t, size: size_t) -> c_int {
	shallow_call(|funs| (funs.posix_memalign)(addr, align, size))
}

#[no_mangle]
pub unsafe extern "C" fn free(addr: *mut c_void) {
	shallow_call(|funs| (funs.free)(addr));
}

/// The `Funs` singleton and its `shallow_call()` accessor.
mod funs {
	use dlfcn::Handle;
	use dlfcn::dlsym;
	use libc::c_int;
	use libc::c_void;
	use libc::size_t;

	/// Singleton storing the locations of the native functions that we're wrapping.
	///
	/// Obtain a reference using the (high-level) `shallow_call()` function or its (low-level)
	/// `funs()` helper.
	pub struct Funs {
		pub malloc: unsafe extern "C" fn(size_t) -> *mut c_void,
		pub calloc: unsafe extern "C" fn(size_t, size_t) -> *mut c_void,
		pub realloc: unsafe extern "C" fn(*mut c_void, size_t) -> *mut c_void,
		pub posix_memalign: unsafe extern "C" fn(*mut *mut c_void, size_t, size_t) -> c_int,
		pub free: unsafe extern "C" fn(*mut c_void),
		_singleton: (),
	}

	/// "Thunk," guarding against mutual recursion.
	///
	/// We have a bootstrapping crisis: `dlsym()` allocates memory during the initialization of `FUNS`!
	/// Clearly we can't permit the nested call because it, too, would find `FUNS` uninitialized and
	/// incite infinite recursion.  Fortunately, glibc's `dlsym()` implementation gracefully falls back
	/// to using a static buffer if its allocation request fails (see `dlerror.c:_dlerror_run()`),
	/// behavior that we can exploit by returning `NULL` upon detecting (potentially) mutual recursion.
	/// The only sticky situation to handle is threads that simultaneously attempt to allocate memory
	/// *for the first time throughout the entire program*.  Fortunately, the `funs()` function guards
	/// against just this scenario, so long as we're careful not to allow it to deadlock via multiple
	/// invocations on the same thread.
	pub fn shallow_call<T: Optional, F: Fn(&'static Funs) -> T>(thunk: F) -> T {
		use std::cell::Cell;

		thread_local! {
			static RECURSING: Cell<bool> = Cell::new(false);
		}

		if RECURSING.with(|recursing| recursing.replace(true)) {
			T::none()
		} else {
			let res = thunk(funs());
			RECURSING.with(|recursing| recursing.set(false));
			res
		}
	}

	// It would be nice to reuse Default here; however, it isn't implemented for pointers, and the type
	// system doesn't (yet?) support trait specialization.
	/// A value that would result from thunking, assuming we wound up doing so.
	pub trait Optional {
		/// The "default" value assumed if we *don't* thunk.
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

	/// Obtain a reference to the `Funs` singleton.
	///
	/// This function is thread safe: concurrent calls will block.  Note that mutually-recursive
	/// invocation consitutes a deadlock!
	fn funs() -> &'static Funs {
		use std::sync::ONCE_INIT;
		use std::sync::Once;

		unsafe extern "C" fn malloc(_: size_t) -> *mut c_void {
			unreachable!()
		}
		unsafe extern "C" fn calloc(_: size_t, _: size_t) -> *mut c_void {
			unreachable!()
		}
		unsafe extern "C" fn realloc(_: *mut c_void, _: size_t) -> *mut c_void {
			unreachable!()
		}
		unsafe extern "C" fn posix_memalign(_: *mut *mut c_void, _: size_t, _: size_t) -> c_int {
			unreachable!()
		}
		unsafe extern "C" fn free(_: *mut c_void) {
			unreachable!()
		}

		static mut FUNS: Funs = Funs {
			malloc: malloc,
			calloc: calloc,
			realloc: realloc,
			posix_memalign: posix_memalign,
			free: free,
			_singleton: (),
		};
		static INIT: Once = ONCE_INIT;

		INIT.call_once(|| unsafe {
			FUNS = Funs {
				malloc: dlsym(Handle::next(), b"malloc\0").unwrap().unwrap(),
				calloc: dlsym(Handle::next(), b"calloc\0").unwrap().unwrap(),
				realloc: dlsym(Handle::next(), b"realloc\0").unwrap().unwrap(),
				posix_memalign: dlsym(Handle::next(), b"posix_memalign\0").unwrap().unwrap(),
				free: dlsym(Handle::next(), b"free\0").unwrap().unwrap(),
				_singleton: (),
			};
		});

		unsafe {
			&FUNS
		}
	}
}
