use gotcha::Group;
use gotcha::group_thread_get;
use gotcha::group_thread_set;
use std::cell::RefMut;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::LockResult;
use std::sync::MutexGuard;
use std::thread::AccessError;
use timetravel::Context;

struct ReusableGroup (Group);

impl Deref for ReusableGroup {
	type Target = Group;

	fn deref(&self) -> &Self::Target {
		let ReusableGroup (this) = self;
		this
	}
}

impl Drop for ReusableGroup {
	fn drop(&mut self) {
		// Assert because we should never be finalizing a task during teardown.
		free_groups_handle().unwrap().push(**self);
	}
}

pub struct UntypedContinuation {
	pub thunk: Box<FnMut()>,
	pub nested: Option<Vec<UntypedContinuation>>,
	pub time_limit: u64,
	pub time_out: u64,
	pub pause_resume: Context<Box<[u8]>>,
	group: ReusableGroup,
}

impl UntypedContinuation {
	pub fn new<T: 'static + FnMut()>(thunk: T, timeout: u64, context: Context<Box<[u8]>>) -> Self {
		Self {
			thunk: Box::new(thunk),
			nested: None,
			time_limit: timeout,
			time_out: 0,
			pause_resume: context,

			// Assert because we should never be launch()'ing a task during teardown.
			group: ReusableGroup (free_groups_handle().unwrap().pop().or_else(|| Group::new())
				.expect("Number of libinger tasks exceeds libgotcha groups limit")),
		}
	}
}

// The RefMut automagically prevents Send'ing or Sync'hronizing this thread-local instance.
pub struct CallStack<'a> (Option<RefMut<'a, Vec<UntypedContinuation>>>);

impl CallStack<'static> {
	/// Returns a guard that holds preemption disabled throughout its lifetime.
	pub fn lock() -> Self {
		group_thread_set!(Group::SHARED);

		// Assert because we should never find ourselves lock()'ing during teardown.
		CallStack (Some(call_stack_handle().unwrap()))
	}

	/// Similar to `lock()`, but only succeeds if the call stack is presently unlocked.
	///
	/// On success, the returned guard will **not** reenable preemption when it dies.
	/// On failure, returns a `Concurrency` error if the call stack is currently `lock()`'d, or
	/// a `Teardown` error if invoked during thread teardown.
	pub unsafe fn preempt() -> Result<RefMut<'static, Vec<UntypedContinuation>>, CallStackError> {
		if group_thread_get!().is_shared() {
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
		let group = self.last().map(|frame| *frame.group);
		self.0.take();
		if let Some(group) = group {
			group_thread_set!(group);
		}
	}
}

pub enum CallStackError {
	Concurrency,
	Teardown(AccessError),
}

pub trait CallStackLock {
	fn lock(&self) {
		group_thread_set!(Group::SHARED);
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

fn free_groups_handle() -> LockResult<MutexGuard<'static, Vec<Group>>> {
	use std::sync::ONCE_INIT;
	use std::sync::Mutex;
	use std::sync::Once;

	static mut FREE_GROUPS: Option<Mutex<Vec<Group>>> = None;
	static INIT: Once = ONCE_INIT;

	INIT.call_once(|| unsafe {
		FREE_GROUPS = Some(Mutex::new(vec![]))
	});
	unsafe {
		FREE_GROUPS.as_ref().unwrap().lock()
	}
}
