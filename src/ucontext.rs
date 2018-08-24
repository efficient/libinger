use id::Id;
use libc::ucontext_t;
use stable::StableMutAddr;
use std::cell::RefCell;
use std::io::Error;
use std::io::Result;
use std::ops::DerefMut;
use uninit::Uninit;
use void::Void;

pub struct Context<S: DerefMut<Target = [u8]>> {
	id: Id,
	context: RefCell<ucontext_t>,
	persistent: Option<Persistent<S>>,
}

pub struct HandlerContext (ucontext_t);

struct Persistent<S: DerefMut<Target = [u8]>> {
	stack: S,
	successor: Id,
}

pub fn getcontext<T, A: FnOnce(Context<Void>) -> T, B: FnMut() -> T>(scope: A, checkpoint: B) -> Result<T> {
	unimplemented!()
}

pub fn makecontext<S: DerefMut<Target = [u8]>, F: FnOnce(Context<S>)>(stack: S, gate: F, call: fn()) -> Result<()> {
	unimplemented!()
}

pub fn restorecontext<S: StableMutAddr<Target = [u8]>, F: FnOnce(Context<S>)>(persistent: Context<S>, scope: F) -> Result<()> {
	unimplemented!()
}

#[must_use]
pub fn setcontext<S: DerefMut<Target = [u8]>>(continuation: &Context<S>) -> Option<Error> {
	unimplemented!()
}

impl Context<Void> {
	fn default() -> Self {
		Self::from(None)
	}
}

impl<S: DerefMut<Target = [u8]>> Context<S> {
	fn new(stack: S, successor: Id) -> Self {
		Self::from(Some(Persistent {
			stack,
			successor,
		}))
	}

	fn from(persistent: Option<Persistent<S>>) -> Self {
		Self {
			id: Id::new(),
			context: RefCell::new(ucontext_t::uninit()),
			persistent,
		}
	}

	pub fn swap(&mut self, other: &mut HandlerContext) {
		unimplemented!();
	}
}
