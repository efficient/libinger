//! Interposition on the runtime's dynamic memory allocator for Rust client code.
//!
//! Because Rust client code and any pure Rust libraries used by that code only link against the
//! allocator symbols defined herein, pure Rust programs will not link directly to the C library's
//! own such functions.  This will result in the linker's pruning the latter symbols from this
//! library when it links the binary, which will mean that even weak symbols from the C library are
//! not overridden.  To work around this problem, we also wrap the dynamic allocation functions to
//! which the Rust compiler inserts calls; these serve as entry points from Rust code to complement
//! those defined for C code.
//!
//! **NB: It's important to remember that the C library functions called from each of the wrappers
//! in this module may or may not themselves be wrapped by the `stdlib` module.  As such, any
//! non-bootstrapping work that needs to be interposed must not only be called from both places, but
//! must also be able to detect this scenario (e.g. via a shared thread-local variable) and remain
//! idempotent in case it is called recursively.**
//!
//! What this file truly contains is an "implementation" of the `std::heap::Alloc` trait for
//! implementing custom allocators.  The functions herein are heavily inspired by their counterparts
//! in `liballoc_system`; however they have been ported to the C ABI interface defined in
//! `libstd::heap` to support building with a stable compiler until such time as the Rust-level
//! allocator API is stable.  Note that by explicitly proxying calls to their C runtime equivalents
//! instead of merely forwarding them on to the Rust standard library, we effectively disable
//! jemalloc on Rust installations where it is the untweakable default.

use std::cmp::min;
use std::mem::size_of;
use std::ptr::copy_nonoverlapping;
use std::ptr::null_mut;
use std::ptr::write_bytes;
use libc::c_void;
use libc::calloc;
use libc::free;
use libc::malloc;
use libc::posix_memalign;
use libc::realloc;

/// Wrapper allowing us to interpose on `std::heap::Alloc::alloc`.
#[no_mangle]
pub unsafe extern "C" fn __rdl_alloc(size: usize, align: usize, _: *mut c_void) -> *mut c_void {
	alloc(size, align, || malloc(size), |_| ())
}

/// Wrapper allowing us to interpose on `std::heap::Alloc::alloc_zeroed`.
#[no_mangle]
pub unsafe extern "C" fn __rdl_alloc_zeroed(size: usize, align: usize, _: *mut c_void) -> *mut c_void {
	alloc(size, align, || calloc(size, 1), |addr| write_bytes(addr, 0, size))
}

/// Wrapper allowing us to interpose on `std::heap::Alloc::realloc`.
#[no_mangle]
pub unsafe extern "C" fn __rdl_realloc(old_addr: *mut c_void, old_size: usize, old_align: usize, new_size: usize, new_align: usize, _: *mut c_void) -> *mut c_void {
	if new_align == old_align {
		alloc(new_size, new_align, || realloc(old_addr, new_size), |new_addr| {
			let size = min(old_size, new_size);
			copy_nonoverlapping(old_addr, new_addr, size);
			free(old_addr);
		})
	} else {
		null_mut()
	}
}

/// Wrapper allowing us to interpose on `std::heap::Alloc::dealloc`.
#[no_mangle]
pub unsafe extern "C" fn __rdl_dealloc(addr: *mut c_void, _: usize, _: usize) {
	free(addr);
}

/// Helper for the allocation functions.
fn alloc<T: Fn() -> *mut c_void, F: Fn(*mut c_void)>(size: usize, align: usize, aligned: T, unaligned: F) -> *mut c_void {
	if align <= size_of::<*const c_void>() {
		aligned()
	} else {
		let mut addr = null_mut();
		if unsafe {
			posix_memalign(&mut addr, align, size) == 0
		} {
			unaligned(addr);
		}
		addr
	}
}
