use crate::reusable::ReusableSync;
use crate::tcb::ThreadControlBlock;

pub fn alloc_localstore() -> ReusableSync<'static, Option<ThreadControlBlock>> {
	use crate::compile_assert::assert_sync;
	use crate::reusable::SyncPool;

	use gotcha::Group;
	use std::convert::TryInto;
	use std::sync::Once;

	static mut LOCALSTORES: Option<SyncPool<Option<ThreadControlBlock>>> = None;
	static INIT: Once = Once::new();
	INIT.call_once(|| {
		let localstores: fn() -> _ = || Some(Some(ThreadControlBlock::new()));
		let localstores = SyncPool::new(localstores);
		localstores.prealloc(Group::limit())
			.expect("libinger: TCB allocator lock was poisoned during init");
		unsafe {
			LOCALSTORES.replace(localstores);
		}
	});

	let localstores = unsafe {
		LOCALSTORES.as_ref()
	}.unwrap();
	assert_sync(&localstores);
	localstores.try_into().expect("libinger: TCB allocator lock is poisoned")
}
