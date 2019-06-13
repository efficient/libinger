#![cfg_attr(bench, feature(test))]

extern crate gotcha;
extern crate libc;
#[cfg(bench)]
extern crate test;
extern crate timetravel;

mod compile_assert;
mod continuation;
mod dlfcn;
mod guard;
mod linger;
mod pthread;
mod signal;
mod stdlib;
mod time;
mod zeroable;

pub use linger::*;

#[cfg(test)]
mod tests {}
