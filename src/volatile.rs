pub struct VolBool (bool);

impl VolBool {
	pub fn new(val: bool) -> Self {
		VolBool (val)
	}

	#[inline]
	pub fn read(&self) -> bool {
		use std::ptr::read_volatile;

		let VolBool (this) = self;
		unsafe {
			read_volatile(this)
		}
	}

	#[inline]
	pub fn write(&mut self, val: bool) {
		use std::ptr::write_volatile;

		let VolBool (this) = self;
		unsafe {
			write_volatile(this, val);
		}
	}
}
