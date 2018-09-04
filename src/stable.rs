//! Markers for allocation structures that never change the addresses of their owned members.

use std::cell::Ref;
use std::cell::RefMut;
use std::ops::Deref;
use std::ops::DerefMut;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::MutexGuard;
use std::sync::RwLockReadGuard;
use std::sync::RwLockWriteGuard;

pub unsafe trait StableAddr: Deref {}
pub unsafe trait StableMutAddr: StableAddr + DerefMut {}

unsafe impl<T: ?Sized> StableAddr for Box<T> {}
unsafe impl<T: ?Sized> StableMutAddr for Box<T> {}

unsafe impl<T: ?Sized> StableAddr for Rc<T> {}

unsafe impl<T: ?Sized> StableAddr for Arc<T> {}

unsafe impl<'a, T: ?Sized> StableAddr for Ref<'a, T> {}

unsafe impl<'a, T: ?Sized> StableAddr for RefMut<'a, T> {}
unsafe impl<'a, T: ?Sized> StableMutAddr for RefMut<'a, T> {}

unsafe impl<'a, T: ?Sized> StableAddr for MutexGuard<'a, T> {}
unsafe impl<'a, T: ?Sized> StableMutAddr for MutexGuard<'a, T> {}

unsafe impl<'a, T: ?Sized> StableAddr for RwLockReadGuard<'a, T> {}

unsafe impl<'a, T: ?Sized> StableAddr for RwLockWriteGuard<'a, T> {}
unsafe impl<'a, T: ?Sized> StableMutAddr for RwLockWriteGuard<'a, T> {}
