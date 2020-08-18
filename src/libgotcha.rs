pub use self::imp::*;

mod prelude {
	pub use libc::pthread_t;
	pub use std::os::raw::c_int;
}

#[cfg(feature = "libgotcha")]
mod imp {
	use super::prelude::*;

	extern {
		pub fn libgotcha_pthread_kill(_: pthread_t, _: c_int) -> c_int;
	}
}

#[cfg(not(feature = "libgotcha"))]
mod imp {
	use super::prelude::*;

	#[inline]
	pub unsafe fn libgotcha_pthread_kill(thread: pthread_t, sig: c_int) -> c_int {
		use libc::pthread_kill;
		pthread_kill(thread, sig)
	}
}
