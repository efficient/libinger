use crate::goot::plot;
use crate::handle::handle;
use std::sync::RwLock;

fn tables() -> &'static RwLock<Vec<&'static plot>> {
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	static mut TABLES: Option<RwLock<Vec<&plot>>> = None;
	static INIT: Once = ONCE_INIT;
	INIT.call_once(|| unsafe {
		TABLES.get_or_insert(RwLock::default());
	});
	unsafe {
		TABLES.as_ref()
	}.unwrap()
}

#[no_mangle]
pub extern "C" fn plot_insert_lib(h: Option<&mut handle>) -> *const plot {
	use crate::goot::goot_empty;
	use crate::goot::goot_insert_lib;
	use std::ptr::null;

	extern "C" {
		fn plot_alloc() -> Option<&'static mut plot>;
	}

	let h = h.unwrap();
	assert!(! h.shadow.is_null());

	let t = unsafe {
		&mut (*h.shadow).override_table
	};
	let lock = tables().read().unwrap();
	if *t != usize::max_value() {
		lock[*t]
	} else {
		let mut fits = true;
		for table in 0..lock.len() {
			if unsafe {
				goot_insert_lib(lock[table].goot, h as *mut _ as _)
			} {
				*t = table;
				break;
			} else if unsafe {
				goot_empty(lock[table].goot)
			} {
				fits = false;
				break;
			}
		}

		if fits {
			if *t == usize::max_value() {
				drop(lock);

				let mut lock = tables().write().unwrap();
				let plot = unsafe {
					plot_alloc()
				}.unwrap();
				fits = unsafe {
					goot_insert_lib(plot.goot, h as *mut _ as _)
				};
				if fits {
					*t = lock.len();
				}
				lock.push(plot);
				if fits {
					lock[*t]
				} else {
					null()
				}
			} else {
				lock[*t]
			}
		} else {
			null()
		}
	}
}

#[no_mangle]
pub extern "C" fn plot_remove_lib(h: Option<&mut handle>) {
	use crate::goot::goot_remove_lib;
	use std::os::raw::c_uint;

	let h = h.unwrap();
	if ! h.shadow.is_null() {
		let table = unsafe {
			&mut (*h.shadow).override_table
		};
		let index = unsafe {
			&mut (*h.shadow).first_entry
		};
		let tables = tables().read().unwrap();
		assert!(unsafe {
			goot_remove_lib(tables[*table].goot, *index)
		});
		*table = usize::max_value();
		debug_assert!(*index == c_uint::max_value());
	}
}
