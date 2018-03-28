use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::ops::Deref;
use std::ops::DerefMut;

pub trait AsBorrowedRef<'a> {
	type Value;
	type Reference: Deref<Target = Self::Value>;

	fn borrow(&'a self) -> Self::Reference;
}

pub trait AsBorrowedMut<'a>: AsBorrowedRef<'a> {
	type MutableReference: DerefMut<Target = Self::Value>;

	fn borrow_mut(&'a self) -> Self::MutableReference;
}

impl<'a, T: 'a> AsBorrowedRef<'a> for RefCell<T> {
	type Value = T;
	type Reference = Ref<'a, T>;

	fn borrow(&'a self) -> Self::Reference {
		RefCell::borrow(self)
	}
}

impl<'a, T: 'a> AsBorrowedMut<'a> for RefCell<T> {
	type MutableReference = RefMut<'a, T>;

	fn borrow_mut(&'a self) -> Self::MutableReference {
		RefCell::borrow_mut(self)
	}
}
