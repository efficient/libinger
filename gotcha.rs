mod libgotcha_api;

use crate::libgotcha_api::LIBGOTCHA_GROUP_ERROR;
use crate::libgotcha_api::LIBGOTCHA_GROUP_SHARED;
use crate::libgotcha_api::libgotcha_group_t;
use std::ops::Deref;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Group (libgotcha_group_t);

impl Group {
	pub const SHARED: Self = Group (LIBGOTCHA_GROUP_SHARED as _);

	#[doc(hidden)]
	pub const _ERROR: Self = Group (LIBGOTCHA_GROUP_ERROR as _);

	pub fn new() -> Option<Self> {
		use crate::libgotcha_api::libgotcha_group_new;
		let this = Group (unsafe {
			libgotcha_group_new()
		});
		if this != Self::_ERROR {
			Some(this)
		} else {
			None
		}
	}

	pub fn is_shared(&self) -> bool {
		self == &Self::SHARED
	}
}

impl Deref for Group {
	type Target = libgotcha_group_t;

	fn deref(&self) -> &Self::Target {
		let Group (this) = self;
		this
	}
}

#[macro_export]
macro_rules! group_thread_get {
	() => (crate::gotcha::_group_thread_accessor()(crate::gotcha::Group::_ERROR));
}

#[macro_export]
macro_rules! group_thread_set {
	( $group:expr ) => (crate::gotcha::_group_thread_accessor()($group));
}

#[doc(hidden)]
pub fn _group_thread_accessor() -> extern fn(Group) -> Group {
	use std::mem::transmute;
	extern {
		fn libgotcha_group_thread_accessor() -> extern fn(Group) -> Group;
	}
	unsafe {
		transmute(libgotcha_group_thread_accessor())
	}
}

pub fn shared_hook(callback: extern fn()) {
	use crate::libgotcha_api::libgotcha_shared_hook;
	unsafe {
		libgotcha_shared_hook(Some(callback));
	}
}
