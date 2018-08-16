use libc::uintptr_t;

pub fn sp() -> uintptr_t {
	#[link(name = "sp")]
	extern "C" {
		fn sp() -> uintptr_t;
	}

	unsafe {
		sp()
	}
}
