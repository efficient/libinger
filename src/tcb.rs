#![allow(unused)]

use gotcha::prctl::ARCH_GET_CPUID;
use gotcha::prctl::ARCH_GET_FS;
use gotcha::prctl::ARCH_GET_GS;
use gotcha::prctl::ARCH_SET_CPUID;
use gotcha::prctl::ARCH_SET_FS;
use gotcha::prctl::ARCH_SET_GS;
use gotcha::Group;
use std::io::Error;
use std::io::Result;
use std::os::raw::c_int;
use std::os::raw::c_ulong;

#[must_use]
pub struct ThreadControlBlock (Option<MaybeMut<'static>>);

impl ThreadControlBlock {
	pub fn current() -> Result<Self> {
		unsafe {
			arch_prctl_get(GetOp::Fs).map(|fs| Self (Some(MaybeMut::Ref(fs))))
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
		Self (Some(MaybeMut::Mut(unsafe {
			&mut *auto
		})))
	}

	pub unsafe fn install(mut self, group: Group) -> Result<ThreadControlBlockGuard> {
		let parent = unguarded_parent(self.install_unguarded(group.into()))?;
		Ok(ThreadControlBlockGuard {
			this: self,
			parent,
		})
	}

	unsafe fn install_unguarded(&mut self, group: Option<Group>) -> Result<Option<Self>> {
		use gotcha::group_lookup_symbol_fn;
		use linger::abort;
		use std::slice;
		extern {
			fn __ctype_init();
		}

		const POINTER_GUARD: usize = 6;

		let Self (fs) = self;
		let fs = fs.as_mut().unwrap();
		let mut cur = None;
		let mut custom = false;
		if let MaybeMut::Mut(fs) = fs {
			let fs = unsafe {
				slice::from_raw_parts_mut(*fs, POINTER_GUARD + 1)
			};
			let cur = cur.get_or_insert(Self::current()?);
			let Self (cur) = &cur;
			let cur: &_ = cur.as_ref().unwrap().into();
			let cur = unsafe {
				slice::from_raw_parts(cur, POINTER_GUARD + 1)
			};
			fs[POINTER_GUARD] = cur[POINTER_GUARD];
			custom = true;
		}

		let fs = (&*fs).into();
		arch_prctl_set(SetOp::Fs, fs)?;
		if custom {
			__ctype_init();
			if let Some(group) = group {
				let __ctype_init: Option<unsafe extern fn()> = group_lookup_symbol_fn!(group, __ctype_init);
				if let Some(__ctype_init) = __ctype_init {
					__ctype_init();
				} else {
					abort("install(): could not get address of __ctype_init()");
				}
			}
		}
		Ok(cur)
	}

	fn take(&mut self) -> Option<MaybeMut<'static>> {
		let Self (this) = self;
		this.take()
	}
}

impl Drop for ThreadControlBlock {
	fn drop(&mut self) {
		let Self (this) = self;
		if let Some(MaybeMut::Mut(_)) = this.as_mut() {
			if let Ok (parent) = unguarded_parent(unsafe {
				self.install_unguarded(None)
			}) {
				drop(ThreadControlBlockGuard {
					this: ThreadControlBlock (self.take()),
					parent,
				});
			} else {
				eprintln!("libinger: could not install TCB to run TLS destructors");
			}
		}
		let Self (fs) = self;
	}
}

fn unguarded_parent(this: Result<Option<ThreadControlBlock>>) -> Result<ThreadControlBlock> {
	this?.ok_or(()).or_else(|_| ThreadControlBlock::current())
}

#[must_use]
pub struct ThreadControlBlockGuard {
	this: ThreadControlBlock,
	parent: ThreadControlBlock,
}

impl ThreadControlBlockGuard {
	pub unsafe fn uninstall(mut self) -> Result<ThreadControlBlock> {
		Ok(ThreadControlBlock (self.this.take()))
	}
}

impl Drop for ThreadControlBlockGuard {
	fn drop(&mut self) {
		extern {
			fn __call_tls_dtors();
			fn _dl_deallocate_tls(_: &mut usize, _: bool);
		}

		let mut dealloc = None;
		if let Some(MaybeMut::Mut(fs)) = self.this.take() {
			unsafe {
				__call_tls_dtors();
			}
			dealloc = Some(fs);
		}
		unsafe {
			self.parent.install_unguarded(None).unwrap();
		}
		if let Some(fs) = dealloc {
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

#[derive(PartialEq)]
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
	use gotcha::install_tcb;
	extern {
		fn libgotcha_arch_prctl(_: c_int, _: c_ulong) -> c_int;
	}

	let val: *const _ = val;
	if op == SetOp::Fs {
		if install_tcb!(val as _) != 0 {
			Err(Error::last_os_error())?;
		}
	} else {
		if libgotcha_arch_prctl(op as _, val as _) != 0 {
			Err(Error::last_os_error())?;
		}
	}

	Ok(())
}
