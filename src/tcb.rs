#![allow(unused)]

use self::imp::ARCH_GET_CPUID;
use self::imp::ARCH_GET_FS;
use self::imp::ARCH_GET_GS;
use self::imp::ARCH_SET_CPUID;
use self::imp::ARCH_SET_FS;
use self::imp::ARCH_SET_GS;
use std::io::Error;
use std::io::Result;
use std::os::raw::c_int;
use std::os::raw::c_ulong;

pub struct ThreadControlBlock (MaybeMut<'static>);

impl ThreadControlBlock {
	pub fn current() -> Result<Self> {
		unsafe {
			arch_prctl_get(GetOp::Fs).map(|fs| Self (MaybeMut::Ref(fs)))
		}
	}

	pub fn new() -> Self {
		extern {
			fn _dl_allocate_tls(_: Option<&mut TCB>) -> Option<&mut TCB>;
		}

		#[repr(C)]
		struct TCB {
			tls_ptr: usize,
			_unused: usize,
			self_ptr: usize,
		}

		let fs = unsafe {
			_dl_allocate_tls(None)
		}.expect("libinger: could not allocate thread-control block");
		let auto: *mut _ = fs;
		fs.tls_ptr = auto as _;
		fs.self_ptr = auto as _;

		let auto: *mut _ = auto as _;
		Self (MaybeMut::Mut(unsafe {
			&mut *auto
		}))
	}

	pub unsafe fn install(&mut self) -> Result<()> {
		use std::slice;

		const POINTER_GUARD: usize = 6;

		let Self (fs) = self;
		if let MaybeMut::Mut(fs) = fs {
			let fs = unsafe {
				slice::from_raw_parts_mut(*fs, POINTER_GUARD + 1)
			};
			let cur = Self::current()?;
			let Self (cur) = &cur;
			let cur: &_ = cur.into();
			let cur = unsafe {
				slice::from_raw_parts(cur, POINTER_GUARD + 1)
			};
			fs[POINTER_GUARD] = cur[POINTER_GUARD];
		}

		let fs = (&*fs).into();
		arch_prctl_set(SetOp::Fs, fs)
	}
}

impl Drop for ThreadControlBlock {
	fn drop(&mut self) {
		extern {
			fn _dl_deallocate_tls(_: &mut usize, _: bool);
		}

		let Self (fs) = self;
		if let MaybeMut::Mut(fs) = fs {
			unsafe {
				_dl_deallocate_tls(fs, true);
			}
		}
	}
}

enum MaybeMut<'a> {
	Ref(&'a usize),
	Mut(&'a mut usize),
}

impl<'a> From<&'a MaybeMut<'a>> for &'a usize {
	fn from(other: &'a MaybeMut) -> Self {
		match other {
		MaybeMut::Ref(other) => other,
		MaybeMut::Mut(other) => other,
		}
	}
}

enum GetOp {
	Cpuid = ARCH_GET_CPUID as _,
	Fs = ARCH_GET_FS as _,
	Gs = ARCH_GET_GS as _,
}

enum SetOp {
	Cpuid = ARCH_SET_CPUID as _,
	Fs = ARCH_SET_FS as _,
	Gs = ARCH_SET_GS as _,
}

unsafe fn arch_prctl_get<'a>(op: GetOp) -> Result<&'a usize> {
	use std::mem::MaybeUninit;
	extern {
		fn arch_prctl(_: c_int, _: *mut c_ulong) -> c_int;
	}

	let mut addr = MaybeUninit::uninit();
	if arch_prctl(op as _, addr.as_mut_ptr()) == 0 {
		let addr: *const _ = addr.assume_init() as _;
		Ok(&*addr)
	} else {
		Err(Error::last_os_error())
	}
}

unsafe fn arch_prctl_set(op: SetOp, val: &usize) -> Result<()> {
	extern {
		fn arch_prctl(_: c_int, _: c_ulong) -> c_int;
	}

	let val: *const _ = val;
	if arch_prctl(op as _, val as _) == 0 {
		Ok(())
	} else {
		Err(Error::last_os_error())
	}
}

mod imp {
	include!(concat!(env!("OUT_DIR"), "/tcb.rs"));
}
