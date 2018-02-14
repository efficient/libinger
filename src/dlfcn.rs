use libc::c_void;
use std::borrow::Cow;

pub enum Handle {}

impl Handle {
	pub fn next() -> *mut Self {
		use libc::RTLD_NEXT;

		RTLD_NEXT as *mut Self
	}
}

pub trait Symbol {
	fn from_void(*mut c_void) -> Self;
}

pub fn dlsym<T: Symbol>(handle: *mut Handle, symbol: &[u8]) -> Result<Option<T>, Cow<str>> {
	use libc::dlsym;

	if *symbol.last().ok_or("symbol must be nonempty")? != b'\0' {
		Err("symbol must be NUL terminated")?
	}

	unsafe {
		use libc::dlerror;
		dlerror();
	}
	let ptr = unsafe {
		dlsym(handle as *mut c_void, symbol.as_ptr() as *const i8)
	};

	if let Some(or) = dlerror() {
		Err(Cow::Owned(or))
	} else {
		Ok(if ptr.is_null() {
			None
		} else {
			Some(T::from_void(ptr))
		})
	}
}

fn dlerror() -> Option<String> {
	use libc::dlerror;
	use std::ffi::CString;

	let msg = unsafe {
		dlerror()
	};

	if msg.is_null() {
		None
	} else {
		Some(unsafe {
			CString::from_raw(msg)
		}.into_string().unwrap_or_else(|err| format!("{}", err)))
	}
}

impl<T> Symbol for *const T {
	fn from_void(ptr: *mut c_void) -> Self {
		ptr as *const c_void as *const T
	}
}

impl<T> Symbol for *mut T {
	fn from_void(ptr: *mut c_void) -> Self {
		ptr as *mut T
	}
}

// NOTE: Replace with a macro to cut down on code duplication.
// It would be nice to be able to reuse a single implementation for functions of any arity; however,
// the type system doesn't (yet?) support it.
impl<T, U> Symbol for unsafe extern "C" fn(T) -> U {
	fn from_void(ptr: *mut c_void) -> Self {
		use std::mem::transmute;

		debug_assert!(!ptr.is_null());
		unsafe {
			transmute(ptr)
		}
	}
}

impl<S, T, U> Symbol for unsafe extern "C" fn(S, T) -> U {
	fn from_void(ptr: *mut c_void) -> Self {
		use std::mem::transmute;

		debug_assert!(!ptr.is_null());
		unsafe {
			transmute(ptr)
		}
	}
}

impl<R, S, T, U> Symbol for unsafe extern "C" fn(R, S, T) -> U {
	fn from_void(ptr: *mut c_void) -> Self {
		use std::mem::transmute;

		debug_assert!(!ptr.is_null());
		unsafe {
			transmute(ptr)
		}
	}
}

#[cfg(test)]
mod tests {
	use dlfcn::*;

	#[test]
	fn dlsym_ctype() {
		let is_a = |fun| {
			let fun: unsafe extern "C" fn(i32) -> i32 = dlsym(Handle::next(), fun).unwrap().unwrap();
			move |chr| unsafe {
				fun(chr as i32)
			} != 0
		};

		let isdigit = is_a(b"isdigit\0");
		assert!( isdigit(b'0'));
		assert!(!isdigit(b'a'));
		assert!(!isdigit(b' '));
		assert!(!isdigit(b'A'));

		let islower = is_a(b"islower\0");
		assert!(!islower(b'0'));
		assert!( islower(b'a'));
		assert!(!islower(b' '));
		assert!(!islower(b'A'));

		let isspace = is_a(b"isspace\0");
		assert!(!isspace(b'0'));
		assert!(!isspace(b'a'));
		assert!( isspace(b' '));
		assert!(!isspace(b'A'));

		let isupper = is_a(b"isupper\0");
		assert!(!isupper(b'0'));
		assert!(!isupper(b'a'));
		assert!(!isupper(b' '));
		assert!( isupper(b'A'));
	}

	#[test]
	fn dlsym_errno() {
		use libc::EINVAL;
		use libc::fopen;

		let errno: *mut i32 = dlsym(Handle::next(), b"errno\0").unwrap().unwrap();

		unsafe {
			*errno = 0;
			fopen(b"\0".as_ptr() as *const i8, b"\0".as_ptr() as *const i8);
		}
		assert_eq!(EINVAL, unsafe {
			*errno
		});
	}
}
