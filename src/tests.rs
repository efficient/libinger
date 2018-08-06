#![doc(test(attr(deny(warnings))))]

//! ```compile_fail
//! use std::mem::uninitialized;
//! use ucontext::Context;
//!
//! fn assert_clone<T: Clone>(_: T) {}
//!
//! let context: Context = unsafe {
//! 	uninitialized()
//! };
//! assert_clone(context);
//! ```

//! ```compile_fail
//! use std::mem::uninitialized;
//! use ucontext::Context;

//! fn assert_send<T: Send>(_: T) {}
//!
//! let context: Context = unsafe {
//! 	uninitialized()
//! };
//! assert_send(context);
//! ```

//! ```compile_fail
//! use std::mem::uninitialized;
//! use ucontext::Context;
//!
//! fn assert_sync<T: Sync>(_: T) {}
//!
//! let context: Context = unsafe {
//! 	uninitialized()
//! };
//! assert_sync(context);
//! ```
