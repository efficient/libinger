use std::cell::BorrowMutError;
use std::cell::RefCell;
use std::cell::RefMut;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ops::DerefMut;
use std::result::Result as StdResult;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;

type Sync<T> = Mutex<Vec<T>>;
type Unsync<T> = RefCell<Vec<T>>;

pub struct Reusable<'a, T, A = Unsync<T>>
where &'a A: SharedMut<Vec<T>> {
	value: Option<T>,
	pool: &'a A,
}

pub type ReusableSync<'a, T> = Reusable<'a, T, Sync<T>>;

impl<'a, T, A, B: Fn() -> Option<T>> TryFrom<&'a Pool<T, B, A>> for Reusable<'a, T, A>
where &'a A: SharedMut<Vec<T>> {
	type Error = Option<<&'a A as SharedMut<Vec<T>>>::Error>;

	fn try_from(pool: &'a Pool<T, B, A>) -> StdResult<Reusable<'a, T, A>, Self::Error> {
		let builder = &pool.builder;
		let pool = &pool.allocated;
		let value = pool.try()?.pop().or_else(builder);
		if value.is_some() {
			Ok(Self {
				value,
				pool,
			})
		} else {
			Err(None)
		}
	}
}

impl<'a, T, A> Deref for Reusable<'a, T, A>
where &'a A: SharedMut<Vec<T>> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// Can only be None if we're called while being dropped!
		self.value.as_ref().unwrap()
	}
}

impl<'a, T, A> DerefMut for Reusable<'a, T, A>
where &'a A: SharedMut<Vec<T>> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// Can only be None if we're called while being dropped!
		self.value.as_mut().unwrap()
	}
}

impl<'a, T, A> Drop for Reusable<'a, T, A>
where &'a A: SharedMut<Vec<T>> {
	fn drop(&mut self) {
		// Can only be None on a double drop!
		let value = self.value.take().unwrap();

		// Panic instead of losing this value.
		self.pool.try().unwrap().push(value)
	}
}

pub struct Pool<T, B: ?Sized = fn() -> Option<T>, A = Unsync<T>> {
	_type: PhantomData<T>,
	allocated: A,
	builder: B,
}

pub type SyncPool<T, B = fn() -> Option<T>> = Pool<T, B, Sync<T>>;

impl<'a, T, F: Fn() -> Option<T>, C: Default + 'a> Pool<T, F, C>
where &'a C: SharedMut<Vec<T>> {
	pub fn new(builder: F) -> Self {
		Self {
			_type: PhantomData::default(),
			allocated: C::default(),
			builder: builder,
		}
	}

	pub fn prealloc(&'a self, count: usize)
	-> StdResult<(), Option<<&'a C as SharedMut<Vec<T>>>::Error>> {
		use std::collections::LinkedList;
		use std::convert::TryInto;

		let swap: LinkedList<StdResult<Reusable<_, _>, _>> = (0..count).map(|_|
			self.try_into()
		).collect();
		for temp in swap {
			temp?;
		}
		Ok(())
	}
}

impl<'a, T: Default, C: Default + 'a> Default for Pool<T, fn() -> Option<T>, C>
where &'a C: SharedMut<Vec<T>> {
	fn default() -> Self {
		Self::new(|| Some(T::default()))
	}
}

pub type Result<'a, T, A = Unsync<T>> = StdResult<
	Reusable<'a, T, A>,
	Option<<&'a A as SharedMut<Vec<T>>>::Error>,
>;

pub type SyncResult<'a, T> = Result<'a, T, Sync<T>>;

#[doc(hidden)]
pub trait SharedMut<T> {
	type Okay: DerefMut<Target = T>;
	type Error: Debug;

	fn try(self) -> StdResult<Self::Okay, Self::Error>;
}

impl<'a, T> SharedMut<T> for &'a RefCell<T> {
	type Okay = RefMut<'a, T>;
	type Error = BorrowMutError;

	fn try(self) -> StdResult<Self::Okay, Self::Error> {
		self.try_borrow_mut()
	}
}

impl<'a, T> SharedMut<T> for &'a Mutex<T> {
	type Okay = MutexGuard<'a, T>;
	type Error = PoisonError<Self::Okay>;

	fn try(self) -> StdResult<Self::Okay, Self::Error> {
		self.lock()
	}
}
