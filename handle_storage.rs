use crate::handle::error;
use crate::handle::handle;
use crate::handle::link_map;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::RwLock;

fn handles() -> &'static RwLock<HashMap<HandleId, Box<(handle, Mutex<()>)>>> {
	use std::sync::ONCE_INIT;
	use std::sync::Once;

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

#[no_mangle]
pub extern "C" fn handle_get(
	obj: *const link_map,
	setup: unsafe extern "C" fn(*mut handle, *const link_map) -> error,
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

		let mut lock = handles().write().unwrap();
		let mut new = false;
		let entry = lock.entry(HandleId (obj)).or_insert_with(|| {
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
	}
}

#[derive(Eq, Hash, PartialEq)]
struct HandleId (*const link_map);
