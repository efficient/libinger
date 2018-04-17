#![allow(dead_code)]

#[inline(always)]
pub fn assert_sync<T: Sync>(_: &T) {}
