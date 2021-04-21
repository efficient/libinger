use crate::reusable::SyncResult;

use gotcha::Group as GotchaGroup;

pub fn assign_group() -> SyncResult<'static, GotchaGroup> {
	use crate::compile_assert::assert_sync;
	use crate::reusable::SyncPool;

	use std::convert::TryInto;
	use std::sync::Once;

	static mut GROUPS: Option<SyncPool<GotchaGroup>> = None;
	static INIT: Once = Once::new();
	INIT.call_once(|| unsafe {
		GROUPS.replace(SyncPool::new(GotchaGroup::new));
	});

	let groups = unsafe {
		GROUPS.as_ref()
	}.unwrap();
	assert_sync(&groups);
	groups.try_into()
}
