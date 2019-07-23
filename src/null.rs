#[inline]
pub fn null_fn_mut() -> *mut impl FnMut() {
	use std::ptr::null_mut;

	fn null() {}

	let null: *mut _ = &mut null;
	if null == null {
		null_mut()
	} else {
		// This unreachable arm is needed to help the compiler infer the unnamable type.
		null
	}
}

