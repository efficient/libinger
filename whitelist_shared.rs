#![allow(improper_ctypes)]

use crate::handle::handle;
use std::collections::HashSet;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::RwLock;

fn whitelist() -> &'static RwLock<HashSet<&'static CStr>> {
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	extern "C" {
		fn whitelist_shared_init(_: *mut HashSet<&CStr>);
	}

	static INIT: Once = ONCE_INIT;
	static mut WHITESET: Option<RwLock<HashSet<&CStr>>> = None;
	INIT.call_once(|| {
		let whiteset = unsafe {
			WHITESET.get_or_insert(RwLock::default())
		};
		let mut whiteset = whiteset.write().unwrap();
		unsafe {
			whitelist_shared_init(&mut *whiteset);
		}
		drop(whiteset);
	});
	unsafe {
		WHITESET.as_ref()
	}.unwrap()
}

#[no_mangle]
pub extern "C" fn whitelist_shared_contains(symbol: *const c_char) -> bool {
	let whitelist = whitelist().read().unwrap();
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

#[no_mangle]
pub extern "C" fn whitelist_so_insert(handle: *const handle) {
	extern "C" {
		fn whitelist_so_insert_with(_: *const handle, _: *mut HashSet<&CStr>);
	}

	let mut whitelist = whitelist().write().unwrap();
	unsafe {
		whitelist_so_insert_with(handle, &mut *whitelist);
	}
	drop(whitelist);
}
