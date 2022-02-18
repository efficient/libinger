mod compile_assert;
pub mod ffi;
pub mod force;
pub mod future;
mod groups;
mod lifetime;
mod linger;
mod localstores;
mod preemption;
pub mod profiler;
mod reusable;
mod signals;
mod stacks;
#[cfg(not(feature = "notls"))]
mod tcb;
mod timer;
mod unfurl;

#[cfg(feature = "notls")]
mod tcbstub;
#[cfg(feature = "notls")]
mod tcb {
	pub use crate::tcbstub::*;
}

pub use linger::*;

use gotcha::Group;

const QUANTUM_MICROSECS: u64  = 100;

#[doc(hidden)]
pub const STACK_N_PREALLOC: usize = Group::LIMIT;
const STACK_SIZE_BYTES: usize = 2 * 1_024 * 1_024;

#[no_mangle]
static libgotcha_exitanalysis: bool = true;

pub fn concurrency_limit() -> usize {
	Group::limit()
}

#[cfg(test)]
fn main() {}
