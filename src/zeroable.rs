use std::mem::zeroed;

pub unsafe trait Zeroable
where Self: Sized {
	fn new() -> Self {
		unsafe {
			zeroed()
		}
	}
}
