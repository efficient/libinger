use std::cell::Cell;
use std::cell::RefMut;
use std::ops::Deref;
use std::ops::DerefMut;
use std::thread::AccessError;
use timetravel::Context;

thread_local! {
	static BLOCK: Cell<bool> = Cell::new(false);
}

pub struct UntypedContinuation {
	pub thunk: Box<FnMut()>,
	pub nested: Option<Vec<UntypedContinuation>>,
	pub time_limit: u64,
	pub time_out: u64,
	pub pause_resume: Context<Box<[u8]>>,
}

impl UntypedContinuation {
	pub fn new<T: 'static + FnMut()>(thunk: T, timeout: u64, context: Context<Box<[u8]>>) -> Self {
		Self {
			thunk: Box::new(thunk),
			nested: None,
			time_limit: timeout,
			time_out: 0,
			pause_resume: context,
		}
	}
}

// The RefMut automagically prevents Send'ing or Sync'hronizing this thread-local instance.
pub struct CallStack<'a> (Option<RefMut<'a, Vec<UntypedContinuation>>>);

impl CallStack<'static> {
	/// Returns a guard that holds preemption disabled throughout its lifetime.
	pub fn lock() -> Self {
		BLOCK.with(|block| block.set(true));

		// Assert because we should never find ourselves lock()'ing during teardown.
		CallStack (Some(call_stack_handle().unwrap()))
	}

	/// Similar to `lock()`, but only succeeds if the call stack is presently unlocked.
	///
	/// On success, the returned guard will **not** reenable preemption when it dies.
	/// On failure, returns a `Concurrency` error if the call stack is currently `lock()`'d, or
	/// a `Teardown` error if invoked during thread teardown.
	pub unsafe fn preempt() -> Result<RefMut<'static, Vec<UntypedContinuation>>, CallStackError> {
		if BLOCK.with(|block| block.get()) {
			Err(CallStackError::Concurrency)?;
		}

		Ok(call_stack_handle().map_err(|or| CallStackError::Teardown(or))?)
	}
}

impl<'a> Deref for CallStack<'a> {
	type Target = Vec<UntypedContinuation>;

	fn deref(&self) -> &Self::Target {
		self.0.as_ref().unwrap()
	}
}

impl<'a> DerefMut for CallStack<'a> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.0.as_mut().unwrap()
	}
}

impl<'a> Drop for CallStack<'a> {
	fn drop(&mut self) {
		self.0.take();
		BLOCK.with(|block| block.set(false));
	}
}

pub enum CallStackError {
	Concurrency,
	Teardown(AccessError),
}

pub trait CallStackLock {
	fn lock(&self) {
		BLOCK.with(|block| block.set(true));
	}
}

impl CallStackLock for Vec<UntypedContinuation> {}

fn call_stack_handle() -> Result<RefMut<'static, Vec<UntypedContinuation>>, AccessError> {
	use std::cell::RefCell;
	use std::mem::transmute;

	thread_local! {
		static CALL_STACK: RefCell<Vec<UntypedContinuation>> = RefCell::new(vec![]);
	}

	let call_stack: &RefCell<_> = CALL_STACK.try_with(|call_stack| unsafe {
		transmute(call_stack)
	})?;
	Ok(call_stack.borrow_mut())
}
