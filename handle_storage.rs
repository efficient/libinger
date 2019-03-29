use crate::handle::error;
use crate::handle::handle;
use crate::handle::link_map;
use std::collections::HashMap;
use std::sync::ONCE_INIT;
use std::sync::Mutex;
use std::sync::Once;
use std::sync::RwLock;
use std::sync::RwLockWriteGuard;

fn handles() -> &'static RwLock<HashMap<HandleId, Box<(handle, Mutex<()>)>>> {
	unsafe impl Send for HandleId {}
	unsafe impl Sync for HandleId {}
	unsafe impl Send for handle {}
	unsafe impl Sync for handle {}

	static mut HANDLES: Option<RwLock<HashMap<HandleId, Box<(handle, Mutex<()>)>>>> = None;
	static INIT: Once = ONCE_INIT;
	INIT.call_once(|| unsafe {
		HANDLES.get_or_insert(RwLock::default());
	});
	unsafe {
		HANDLES.as_ref()
	}.unwrap()
}

fn globals() -> &'static RwLock<HashMap<usize, usize>> {
	static mut STATICS: Option<RwLock<HashMap<usize, usize>>> = None;
	static INIT: Once = ONCE_INIT;
	INIT.call_once(|| unsafe {
		STATICS.get_or_insert(RwLock::default());
	});
	unsafe {
		STATICS.as_ref()
	}.unwrap()
}

#[no_mangle]
pub extern "C" fn handle_get(
	obj: *const link_map,
	setup: Option<unsafe extern "C" fn(*mut handle, *const link_map) -> error>,
	code: Option<&mut error>,
) -> *const handle {
	use std::mem::uninitialized;
	use std::ptr::null;
	use std::sync::atomic::AtomicBool;
	use std::sync::atomic::Ordering;

	let lock = handles().read().unwrap();
	if let Some(entry) = lock.get(&HandleId (obj)) {
		let (handle, init) = &**entry;
		let handle: *const _ = handle;
		let init: *const _ = init;
		drop(lock);
		if let Ok(init) = unsafe {
			(*init).lock()
		} {
			drop(init);
			handle
		} else {
			null()
		}
	} else {
		drop(lock);
		if let Some(setup) = setup {
			let mut res = null();
			let new = AtomicBool::new(false);
			handle_helper(
				|lock| lock.entry(HandleId (obj)).or_insert_with(|| {
					new.store(true, Ordering::Relaxed);
					Box::new((
						unsafe {
							uninitialized()
						},
						Mutex::new(()),
					))
				}),
				|handle| if new.load(Ordering::Relaxed) {
					let status = unsafe {
						setup(handle, obj)
					};
					if let Some(code) = code {
						*code = status;
					}
					res = handle;
				},
			);
			res
		} else {
			null()
		}
	}
}

fn handle_helper<
	G: for<'a> FnOnce(&'a mut RwLockWriteGuard<HashMap<HandleId, Box<(handle, Mutex<()>)>>>) ->
		&'a mut (handle, Mutex<()>),
	O: FnOnce(*mut handle),
>(
	get: G,
	oper: O,
) {
	let mut lock = handles().write().unwrap();
	let (handle, init) = get(&mut lock);
	let handle: *mut _ = handle;
	let init: *const _ = init;
	if let Ok(init) = unsafe {
		(*init).lock()
	} {
		drop(lock);
		oper(handle);
		drop(init);
	}
}

#[no_mangle]
pub extern "C" fn handle_update(obj: *const link_map, oper: unsafe extern "C" fn(*mut handle) -> error) -> error {
	let mut err = error::SUCCESS;
	handle_helper(|lock| lock.get_mut(&HandleId (obj)).unwrap(), |handle| unsafe {
		err = oper(handle);
	});
	err
}

#[no_mangle]
pub extern "C" fn globals_insert(addr: usize, trampoline: usize) -> bool {
	use std::collections::hash_map::Entry;

	if let Entry::Vacant(spot) = globals().write().unwrap().entry(addr) {
		spot.insert(trampoline);
		true
	} else {
		false
	}
}

#[no_mangle]
pub extern "C" fn globals_contains(addr: usize) -> bool {
	globals().read().unwrap().get(&addr).is_some()
}

#[no_mangle]
pub extern "C" fn globals_get(addr: usize) -> usize {
	*globals().read().unwrap().get(&addr).unwrap_or(&0)
}

#[no_mangle]
pub extern "C" fn globals_set(addr: usize, trampoline: usize) {
	globals().write().unwrap().insert(addr, trampoline);
}

#[no_mangle]
pub extern "C" fn globals_remove(addr: usize) -> bool {
	globals().write().unwrap().remove(&addr).is_some()
}

#[derive(Eq, Hash, PartialEq)]
struct HandleId (*const link_map);
