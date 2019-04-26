mod libgotcha_api;

use crate::libgotcha_api::libgotcha_group_t;
use std::ops::Deref;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Group (libgotcha_group_t);

impl Deref for Group {
	type Target = libgotcha_group_t;

	fn deref(&self) -> &Self::Target {
		let Group (this) = self;
		this
	}
}

#[macro_export]
macro_rules! group_thread {
	() => (crate::gotcha::_group_thread_getter()());
}

#[doc(hidden)]
pub fn _group_thread_getter() -> extern fn() -> Group {
	use std::mem::transmute;
	extern {
		fn libgotcha_group_thread_getter() -> extern fn() -> Group;
	}
	unsafe {
		transmute(libgotcha_group_thread_getter())
	}
}

pub fn shared_hook(callback: extern fn()) {
	use crate::libgotcha_api::libgotcha_shared_hook;
	unsafe {
		libgotcha_shared_hook(Some(callback));
	}
}
