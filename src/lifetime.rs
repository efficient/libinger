#![allow(dead_code)]

pub unsafe fn unbound<'a, T>(bounded: *const T) -> &'a T {
	&*bounded
}

pub unsafe fn unbound_mut<'a, T>(bounded: *mut T) -> &'a mut T {
	&mut *bounded
}
