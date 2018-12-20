#[doc(hidden)]
pub mod handle;
#[doc(hidden)]
pub mod handle_storage;
#[doc(hidden)]
pub mod mirror;
#[doc(hidden)]
pub mod whitelist_copy;
#[doc(hidden)]
pub mod whitelist_shared;

pub use crate::mirror::error;
#[doc(hidden)]
pub use crate::mirror::link_map;
use std::ffi::CStr;
use std::ffi::CString;
use std::fmt::Debug;
use std::fmt::Error as FmtError;
use std::fmt::Formatter;
use std::os::raw::c_char;

pub struct Error {
	pub error: error,
	message: &'static CStr,
	explanation: Option<CString>,
}

#[derive(Clone, Copy)]
pub struct ObjectFile (*const link_map);

pub unsafe fn mirror_object_containing<T>(function: &T) -> Result<(), Error> {
	use crate::mirror::mirror_object;
	use std::mem::transmute;

	let mirror_object: unsafe extern "C" fn(_, _) -> _ = mirror_object;
	let mirror_object: extern "C" fn(_, _) -> _ = transmute(mirror_object);
	test_object_containing(mirror_object, function)
}

#[doc(hidden)]
pub fn test_object_containing<T>(
	plugin: extern "C" fn(*const link_map, *const c_char) -> error,
	function: &T
) -> Result<(), Error> {
	use crate::mirror::test_object_containing;

	Error::from(unsafe {
		test_object_containing(Some(plugin), function as *const _ as _)
	})
}

pub unsafe fn mirror_object(object: ObjectFile, path: Option<&CStr>) -> Result<(), Error> {
	use crate::mirror::mirror_object;
	use std::ptr::null;

	let ObjectFile (object) = object;
	Error::from(mirror_object(object, path.map(|path| path.as_ptr()).unwrap_or(null())))
}

impl Error {
	fn from(error: error) -> Result<(), Error> {
		use crate::mirror::error_explanation;
		use crate::mirror::error_message;

		if error == error::SUCCESS {
			Ok(())
		} else {
			let message = unsafe {
				error_message(error)
			};
			let explanation = unsafe {
				error_explanation(error)
			};
			debug_assert!(! message.is_null());

			let message = unsafe {
				CStr::from_ptr(message)
			};
			let explanation = if explanation.is_null() {
				None
			} else {
				Some(unsafe {
					CStr::from_ptr(explanation)
				}.to_owned())
			};
			Err(Self {
				error,
				message,
				explanation,
			})
		}
	}
}

impl Debug for Error {
	fn fmt(&self, form: &mut Formatter) -> Result<(), FmtError> {
		if let Some(explanation) = self.explanation.as_ref() {
			write!(form, "{:?}: {:?}", self.message, explanation)
		} else {
			write!(form, "{:?}", self.message)
		}
	}
}
