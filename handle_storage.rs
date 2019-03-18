use crate::handle::error;
use crate::handle::handle;
use crate::handle::link_map;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::ONCE_INIT;
use std::sync::Mutex;
use std::sync::Once;
use std::sync::RwLock;

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

fn statics() -> &'static RwLock<HashSet<usize>> {
	static mut STATICS: Option<RwLock<HashSet<usize>>> = None;
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
			let mut lock = handles().write().unwrap();
			let mut new = false;
			let entry = lock.entry(HandleId (obj)).or_insert_with(|| {;
				new = true;
				Box::new((
					unsafe {
						uninitialized()
					},
					Mutex::new(()),
				))
			});
			let (handle, init) = &mut **entry;
			let handle: *mut _ = handle;
			let init: *const _ = init;
			if let Ok(init) = unsafe {
				(*init).lock()
			} {
				drop(lock);
				if new {
					let res = unsafe {
						setup(handle, obj)
					};
					if let Some(code) = code {
						*code = res;
					}
				}
				drop(init);
				handle
			} else {
				null()
			}
		} else {
			null()
		}
	}
}

#[no_mangle]
pub extern "C" fn statics_insert(addr: usize) -> bool {
	statics().write().unwrap().insert(addr)
}

#[no_mangle]
pub extern "C" fn statics_contains(addr: usize) -> bool {
	statics().read().unwrap().contains(&addr)
}

#[no_mangle]
pub extern "C" fn statics_remove(addr: usize) -> bool {
	statics().write().unwrap().remove(&addr)
}

#[derive(Eq, Hash, PartialEq)]
struct HandleId (*const link_map);
