extern crate gotcha;
extern crate libc;
extern crate signal;
extern crate timetravel;

mod compile_assert;
pub mod future;
mod groups;
mod lifetime;
mod linger;
mod preemption;
pub mod profiler;
mod reusable;
mod signals;
mod stacks;
mod timer;
mod unfurl;

pub use linger::*;

const QUANTUM_MICROSECS: u64  = 100;

const STACK_N_PREALLOC: usize = 511;
const STACK_SIZE_BYTES: usize = 2 * 1_024 * 1_024;

#[cfg(test)]
fn main() {}
