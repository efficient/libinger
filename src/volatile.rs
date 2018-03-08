use std::ptr::read_volatile;
use std::ptr::write_volatile;

pub struct VolBool (bool);

impl VolBool {
	pub fn new(val: bool) -> Self {
		VolBool (val)
	}

	pub fn set(&mut self, val: bool) {
		unsafe {
			write_volatile(&mut self.0, val);
		}
	}

	pub fn get(&self) -> bool {
		unsafe {
			read_volatile(&self.0)
		}
	}
}
