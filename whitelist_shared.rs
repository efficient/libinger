#![allow(improper_ctypes)]

use std::collections::HashSet;
use std::ffi::CStr;
use std::os::raw::c_char;

fn whitelist() -> &'static HashSet<&'static CStr> {
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	extern "C" {
		fn whitelist_shared_init(out: *mut HashSet<&CStr>);
	}

	static INIT: Once = ONCE_INIT;
	static mut WHITESET: Option<HashSet<&CStr>> = None;
	INIT.call_once(|| {
		let whiteset = unsafe {
			WHITESET.get_or_insert(HashSet::default())
		};
		unsafe {
			whitelist_shared_init(whiteset);
		}
	});
	unsafe {
		WHITESET.as_ref()
	}.unwrap()
}

#[no_mangle]
pub extern "C" fn whitelist_shared_contains(symbol: *const c_char) -> bool {
	let whitelist = whitelist();
	if symbol.is_null() {
		false
	} else {
		whitelist.contains(unsafe {
			CStr::from_ptr(symbol)
		})
	}
}

#[no_mangle]
pub extern "C" fn whitelist_shared_insert(
	whitelist: Option<&mut HashSet<&CStr>>,
	symbol: *const c_char,
) {
	whitelist.unwrap().insert(unsafe {
		CStr::from_ptr(symbol)
	});
}
