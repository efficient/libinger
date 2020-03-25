use std::ops::Deref;
use std::ops::DerefMut;

#[repr(transparent)]
pub struct AssertSend<T> (T);

unsafe impl<T> Send for AssertSend<T> {}

impl<T> AssertSend<T> {
	pub unsafe fn new(t: T) -> Self {
		Self (t)
	}
}

impl<T> Deref for AssertSend<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		let Self (this) = self;
		this
	}
}

impl<T> DerefMut for AssertSend<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		let Self (this) = self;
		this
	}
}
