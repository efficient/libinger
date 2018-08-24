pub unsafe trait Uninit: Sized {
	#[inline]
	fn uninit() -> Self {
		use std::mem::uninitialized;

		unsafe {
			uninitialized()
		}
	}
}
