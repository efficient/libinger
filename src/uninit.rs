pub unsafe trait Uninit: Sized {
	#[inline]
	fn uninit() -> Self {
		use std::mem::MaybeUninit;

		unsafe {
			MaybeUninit::uninit().assume_init()
		}
	}
}
