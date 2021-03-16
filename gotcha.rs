mod libgotcha_api;
mod namespace;
pub mod prctl;

use crate::libgotcha_api::LIBGOTCHA_GROUP_ERROR;
use crate::libgotcha_api::LIBGOTCHA_GROUP_SHARED;
use crate::libgotcha_api::libgotcha_group_t;
use crate::namespace::NUM_SHADOW_NAMESPACES;
use std::ops::Deref;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Group (libgotcha_group_t);

impl Group {
	pub const SHARED: Self = Self (LIBGOTCHA_GROUP_SHARED as _);

	#[doc(hidden)]
	pub const _ERROR: Self = Self (LIBGOTCHA_GROUP_ERROR as _);

	pub const LIMIT: usize = NUM_SHADOW_NAMESPACES as _;

	pub fn limit() -> usize {
		use crate::libgotcha_api::libgotcha_group_limit;
		unsafe {
			libgotcha_group_limit()
		}
	}

	pub fn new() -> Option<Self> {
		use crate::libgotcha_api::libgotcha_group_new;
		let this = Self (unsafe {
			libgotcha_group_new()
		});
		if this != Self::_ERROR {
			Some(this)
		} else {
			None
		}
	}

	#[must_use]
	pub fn renew(&self) -> bool {
		use crate::libgotcha_api::libgotcha_group_renew;
		let Self (this) = self;
		unsafe {
			libgotcha_group_renew(*this)
		}
	}

	pub fn is_shared(&self) -> bool {
		self == &Self::SHARED
	}

	#[doc(hidden)]
	pub fn lookup_symbol<T>(&self, decl: &str, _: &T) -> Option<&T> {
		unsafe {
			self.lookup_symbol_impl(decl)
		}.map(|defn| unsafe {
			&*defn
		})
	}

	#[doc(hidden)]
	pub unsafe fn lookup_symbol_mut<T>(&self, decl: &str, _: &T) -> Option<&mut T> {
		self.lookup_symbol_impl(decl).map(|defn| &mut *defn)
	}

	unsafe fn lookup_symbol_impl<T>(&self, sym: &str) -> Option<*mut T> {
		use crate::libgotcha_api::libgotcha_group_symbol;
		use std::ffi::CString;

		let Self (this) = self;
		let sym = CString::new(sym).ok()?;
		Some(libgotcha_group_symbol(*this, sym.into_raw()) as *mut _)
	}
}

impl Deref for Group {
	type Target = libgotcha_group_t;

	fn deref(&self) -> &Self::Target {
		let Self (this) = self;
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

#[macro_export]
macro_rules! group_lookup_symbol {
	( $group:expr, $symbol:ident ) => ($group.lookup_symbol(stringify!($symbol), &$symbol));
}

#[macro_export]
macro_rules! group_lookup_symbol_mut {
	( $group:expr, $symbol:ident ) => ($group.lookup_symbol_mut(stringify!($symbol), &$symbol));
}

#[macro_export]
macro_rules! group_lookup_symbol_fn {
	( $group:expr, $symbol:ident ) => ({
		let mut symbol = Some($symbol as _);
		symbol = crate::std::mem::transmute($group.lookup_symbol(stringify!($symbol), &$symbol));
		symbol
	});
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
