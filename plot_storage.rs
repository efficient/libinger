use crate::goot::plot;
use crate::handle::handle;
use std::sync::RwLock;

fn tables() -> &'static RwLock<Vec<&'static plot>> {
	use std::sync::Once;

	static mut TABLES: Option<RwLock<Vec<&plot>>> = None;
	static INIT: Once = Once::new();
	INIT.call_once(|| unsafe {
		TABLES.get_or_insert(RwLock::default());
	});
	unsafe {
		TABLES.as_ref()
	}.unwrap()
}

#[no_mangle]
extern fn plot_insert_lib(h: Option<&mut handle>) {
	use crate::goot::goot_empty;
	use crate::goot::goot_insert_lib;

	extern {
		fn plot_alloc() -> Option<&'static mut plot>;
	}

	let mut pages = Vec::new();
	let mut first_new = None;
	let mut last_new = false;
	let mut h = h.unwrap();
	let lock = tables().read().unwrap();
	assert!(h.shadow.override_table == usize::max_value());
	while {
		let mut done = false;
		let page = lock.iter().find(|page| {
			let empty = unsafe {
				goot_empty(page.goot)
			};
			done = unsafe {
				goot_insert_lib(page.goot, h as *mut _ as _, pages.len())
			};

			let found = done || empty;
			last_new = false;

			found
		}).map(|deref| *deref).unwrap_or_else(|| {
			let page = unsafe {
				plot_alloc()
			}.unwrap();
			done = unsafe {
				goot_insert_lib(page.goot, h as *mut _ as _, pages.len())
			};

			if first_new.is_none() {
				first_new = Some(pages.len());
			}
			last_new = true;

			page
		});
		pages.push(page);

		! done
	} {}

	drop(lock);
	if let Some(first_new) = first_new {
		let last_new = if last_new {
			pages.len()
		} else {
			pages.len() - 1
		};
		let mut lock = tables().write().unwrap();

		for index in first_new..last_new {
			unsafe {
				(*pages[index].goot).identifier = lock.len();
			}
			lock.push(pages[index]);
		}
	}

	if let Some(last) = pages.last() {
		h.shadow.override_table = unsafe {
			(*last.goot).identifier
		};

		let pages = Box::leak(pages.into_boxed_slice());
		let ptr = pages.as_ptr();
		debug_assert!(ptr == pages as *const _ as _);
		h.plots = ptr as _;
	}
}

#[no_mangle]
extern fn plot_remove_lib(h: Option<&mut handle>) {
	use crate::goot::goot_remove_lib;
	use std::os::raw::c_uint;
	use std::ptr::null_mut;

	let h = h.unwrap();
	let table = &mut h.shadow.override_table;
	let index = &mut h.shadow.first_entry;
	let pages: *mut &[&plot] = h.plots as _;
	let pages = unsafe {
		Box::from_raw(pages)
	};

	for page in pages.iter() {
		let pos;
		if unsafe {
			(*page.goot).identifier
		} == *table && *index != c_uint::max_value() {
			pos = *index;
		} else {
			pos = 0;
		}

		unsafe {
			goot_remove_lib(page.goot, pos);
		}
	}

	h.plots = null_mut();
	*table = usize::max_value();
	debug_assert!(*index == c_uint::max_value());
}
