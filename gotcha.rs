mod libgotcha_api;

use std::ops::Deref;

#[repr(transparent)]
pub struct Group (u8);

impl Deref for Group {
	type Target = u8;

	fn deref(&self) -> &Self::Target {
		let Group (this) = self;
		this
	}
}

pub fn thread_group_getter() -> extern fn() -> Group {
	use crate::libgotcha_api::libgotcha_thread_group_getter;
	use std::mem::transmute;
	unsafe {
		transmute(libgotcha_thread_group_getter().unwrap())
	}
}

pub fn shared_hook(callback: extern fn()) {
	use crate::libgotcha_api::libgotcha_shared_hook;
	unsafe {
		libgotcha_shared_hook(Some(callback));
	}
}
