pub use self::imp::*;

#[cfg(debug_assertions)]
mod imp {
	use std::cell::Cell;
	use std::cell::RefCell;
	use std::marker::PhantomData;

	thread_local! {
		static IDS: RefCell<Vec<usize>> = RefCell::new(Vec::new());
		static SERIALS: Cell<usize> = Cell::new(0);
	}

	#[derive(Clone, Copy)]
	pub struct Id {
		index: usize,
		serial: usize,
		nonsend_nonsync: PhantomData<*const ()>,
	}

	impl Id {
		pub fn new() -> Self {
			let serial = SERIALS.with(|serials| {
				let serial = serials.get();
				serials.set(serial + 1);
				serial
			});

			let index = IDS.with(|ids| {
				let mut ids = ids.borrow_mut();
				let index = ids.len();
				ids.push(serial);
				index
			});

			Self {
				index,
				serial,
				nonsend_nonsync: PhantomData::default(),
			}
		}

		pub fn is_valid(&self) -> bool {
			let id = IDS.with(|ids| ids.borrow().get(self.index).cloned());
			id.map(|serial| serial == self.serial).unwrap_or(false)
		}

		pub fn invalidate(&self) {
			IDS.with(|ids| ids.borrow_mut().truncate(self.index))
		}

		pub fn invalidate_subsequent(&self) {
			IDS.with(|ids| ids.borrow_mut().truncate(self.index + 1))
		}
	}
}

#[cfg(not(debug_assertions))]
mod imp {
	#[derive(Clone, Copy)]
	pub struct Id ();

	impl Id {
		pub fn new() -> Self { Self () }
		pub fn is_valid(&self) -> bool { true }
		pub fn invalidate(&self) {}
		pub fn invalidate_subsequent(&self) {}
	}
}
