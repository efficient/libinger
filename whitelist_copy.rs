use std::collections::HashSet;
use std::os::raw::c_char;

const WHITELIST: [&[u8]; 14] = [
	// libc:
	b"program_invocation_name",
	b"program_invocation_short_name",
	b"stderr",
	b"stdin",
	b"stdout",
	b"__progname",
	b"__progname_full",

	// libstdc++:
	b"_ZSt4cerr",
	b"_ZSt3cin",
	b"_ZSt4clog",
	b"_ZSt4cout",
	b"_ZSt4wcin",
	b"_ZSt5wclog",
	b"_ZSt5wcout",
];

fn whitelist() -> &'static HashSet<&'static [u8]> {
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	static INIT: Once = ONCE_INIT;
	static mut WHITESET: Option<HashSet<&[u8]>> = None;
	INIT.call_once(|| unsafe {
		WHITESET.get_or_insert(WHITELIST.iter().map(|deref| *deref).collect());
	});
	unsafe {
		WHITESET.as_ref()
	}.unwrap()
}

#[no_mangle]
pub extern "C" fn whitelist_copy_contains(symbol: *const c_char) -> bool {
	use std::ffi::CStr;

	let symbol = unsafe {
		CStr::from_ptr(symbol)
	}.to_bytes();
	whitelist().contains(symbol)
}
