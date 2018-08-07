pub struct VolBool (bool);

impl VolBool {
	pub fn new(val: bool) -> Self {
		VolBool (val)
	}

	pub fn load(&self) -> bool {
		use std::ptr::read_volatile;

		let VolBool (val) = self;
		unsafe {
			read_volatile(val)
		}
	}

	pub fn store(&mut self, val: bool) {
		use std::ptr::write_volatile;

		let VolBool (ue) = self;
		unsafe {
			write_volatile(ue, val);
		}
	}
}
