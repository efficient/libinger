pub unsafe trait Uninit: Sized {
	fn uninit() -> Self {
		use std::mem::uninitialized;

		unsafe {
			uninitialized()
		}
	}
}
