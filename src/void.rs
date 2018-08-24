use std::ops::Deref;
use std::ops::DerefMut;

pub enum Void {}

impl Deref for Void {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		unreachable!()
	}
}

impl DerefMut for Void {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unreachable!()
	}
}
