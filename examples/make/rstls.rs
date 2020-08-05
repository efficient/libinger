#![crate_type = "dylib"]

extern crate gotcha;

use std::ffi::c_void;
extern {
	fn libgotcha_tls_get_addr(_: usize) -> Option<&'static c_void>;
}

#[no_mangle]
unsafe extern fn assert_static_repl() {
	// Only libgotcha's static replacement can tolerate a null argument without crashing.
	libgotcha_tls_get_addr(0);
}
