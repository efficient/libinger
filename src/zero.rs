pub unsafe trait Zero: Sized {
	#[inline]
	fn zero() -> Self {
		use std::mem::zeroed;

		unsafe {
			zeroed()
		}
	}
}
