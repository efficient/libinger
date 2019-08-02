pub trait Unfurl<T> {
	unsafe fn unfurl(self) -> T;
}

impl<T> Unfurl<T> for Option<T> {
	unsafe fn unfurl(self) -> T {
		use std::hint::unreachable_unchecked;

		if let Some(t) = self {
			t
		} else {
			unreachable_unchecked()
		}
	}
}
