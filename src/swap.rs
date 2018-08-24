pub trait Swap: Sized {
	fn swap(&mut self, other: &mut Self) {
		use std::mem::swap;

		swap(self, other);
	}
}
