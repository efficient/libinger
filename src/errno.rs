use std::os::raw::c_int;

pub fn errno() -> &'static mut c_int {
	use libc::__errno_location;

	unsafe {
		&mut *__errno_location()
	}
}
