use std::marker::PhantomData;
use std::ops::Deref;
use std::ops::DerefMut;
use super::STACK_SIZE_BYTES;
use timetravel::stable::StableAddr;
use timetravel::stable::StableMutAddr;
use reusable::ReusableSync;

pub fn alloc_stack() -> ReusableSync<'static, Box<[u8]>> {
	use compile_assert::assert_sync;
	use reusable::SyncPool;
	use std::convert::TryInto;
	use std::sync::Once;

	static mut STACKS: Option<SyncPool<Box<[u8]>>> = None;
	static INIT: Once = Once::new();
	INIT.call_once(|| unsafe {
		STACKS.replace(SyncPool::new(|| Some(vec![0; STACK_SIZE_BYTES].into_boxed_slice())));
	});

	let stacks = unsafe {
		STACKS.as_ref()
	}.unwrap();
	assert_sync(&stacks);
	stacks.try_into().expect("libinger: stack allocator lock is poisoned")
}

pub struct DerefAdapter<'a, T> (T, PhantomData<&'a ()>);

impl<T> From<T> for DerefAdapter<'_, T> {
	fn from(t: T) -> Self {
		Self (t, PhantomData::default())
	}
}

impl<'a, T: Deref<Target = U>, U: Deref<Target = V> + 'a, V: ?Sized> Deref for DerefAdapter<'a, T> {
	type Target = V;

	fn deref(&self) -> &Self::Target {
		let Self (t, _) = self;
		&***t
	}
}

impl<'a, T: DerefMut<Target = U>, U: DerefMut<Target = V> + 'a, V: ?Sized> DerefMut for DerefAdapter<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		let Self (t, _) = self;
		&mut ***t
	}
}

unsafe impl<'a, T: Deref<Target = U>, U: StableAddr + 'a> StableAddr for DerefAdapter<'a, T> {}
unsafe impl<'a, T: DerefMut<Target = U>, U: StableMutAddr + 'a> StableMutAddr for DerefAdapter<'a, T> {}
