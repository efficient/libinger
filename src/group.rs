use gotcha::Group as GotchaGroup;
use reusable::SyncResult;

pub fn assign_group() -> SyncResult<'static, GotchaGroup> {
	use compile_assert::assert_sync;
	use reusable::SyncPool;
	use std::convert::TryInto;
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	static mut GROUPS: Option<SyncPool<GotchaGroup, fn() -> Option<GotchaGroup>>> = None;
	static INIT: Once = ONCE_INIT;
	INIT.call_once(|| unsafe {
		GROUPS.replace(SyncPool::new(GotchaGroup::new));
	});

	let groups = unsafe {
		GROUPS.as_ref()
	}.unwrap();
	assert_sync(&groups);
	groups.try_into()
}
