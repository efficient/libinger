#![allow(dead_code)]

//! Doc tests.
//!
//! ```compile_fail
//! extern crate libc;
//! extern crate timetravel;
//!
//! fn restore_expired() {
//!	use libc::MINSIGSTKSZ;
//!	use timetravel::makecontext;
//!	use timetravel::restorecontext;
//!
//!	let mut stack = [0u8; MINSIGSTKSZ];
//!	let mut context = None;
//!	makecontext(&mut stack[..], |thing| context = Some(thing), || unreachable!()).unwrap();
//!	restorecontext(context.unwrap(), |_| unreachable!());
//! }
//! ```
//!
//! ```compile_fail
//! let context: timetravel::Context = unsafe { std::mem::uninitialized() };
//! timetravel::tests::assert_clone(&context)
//! ```
//!
//! ```compile_fail
//! let context: timetravel::Context = unsafe { std::mem::uninitialized() };
//! timetravel::tests::assert_send(&context)
//! ```
//!
//! ```compile_fail
//! let context: timetravel::Context = unsafe { std::mem::uninitialized() };
//! timetravel::tests::assert_sync(&context)
//! ```
//!
//! ```no_run
//! timetravel::tests::assert_copy(&timetravel::id::Id::new())
//! ```
//!
//! ```compile_fail
//! timetravel::tests::assert_send(&timetravel::id::Id::new())
//! ```
//!
//! ```compile_fail
//! timetravel::tests::assert_sync(&timetravel::id::Id::new())
//! ```

#[inline]
pub fn assert_clone<T: Clone>(_: &T) {}

#[inline]
pub fn assert_copy<T: Copy>(_: &T) {}

#[inline]
pub fn assert_send<T: Send>(_: &T) {}

#[inline]
pub fn assert_sync<T: Sync>(_: &T) {}
