pub unsafe trait Zero: Sized {
	fn zero() -> Self {
		use std::mem::zeroed;

		unsafe {
			zeroed()
		}
	}
}
