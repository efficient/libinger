pub unsafe fn unbound<'a, 'b, T: ?Sized>(ptr: &'a T) -> &'b T {
	let ptr: *const _ = ptr;
	&*ptr
}

pub unsafe fn unbound_mut<'a, 'b, T: ?Sized>(ptr: &'a mut T) -> &'b mut T {
	let ptr: *mut _ = ptr;
	&mut *ptr
}
