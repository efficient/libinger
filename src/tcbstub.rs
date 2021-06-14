use gotcha::Group;
use std::io::Result;

#[must_use]
pub struct ThreadControlBlock;

impl ThreadControlBlock {
	pub fn new() -> Self { Self }
	pub unsafe fn install(self, _: Group) -> Result<ThreadControlBlockGuard> { Ok(ThreadControlBlockGuard) }
}

impl Drop for ThreadControlBlock {
	fn drop(&mut self) {}
}

#[must_use]
pub struct ThreadControlBlockGuard;

impl ThreadControlBlockGuard {
	pub unsafe fn uninstall(self) -> Result<ThreadControlBlock> { Ok(ThreadControlBlock) }
}

impl Drop for ThreadControlBlockGuard {
	fn drop(&mut self) {}
}
