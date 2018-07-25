use guard::PreemptGuard;
use libc::ucontext_t;
use std::cell::RefCell;
use std::cell::RefMut;
use std::io::Error;
use std::ops::Deref;
use std::ops::DerefMut;
use std::thread::AccessError;

const STACK_SIZE_BYTES: usize = 2 * 1_024 * 1_024;

pub struct UntypedContinuation {
	pub thunk: Box<FnMut()>,
	pub time_limit: u64,
	pub time_out: u64,
	pub pause_resume: Box<ucontext_t>,
	pub stack: Box<[u8]>,
}

impl UntypedContinuation {
	pub fn new<T: 'static + FnMut()>(thunk: T, timeout: u64, context: ucontext_t) -> Self {
		use ucontext::fixupcontext;

		let mut context = Box::new(context);
		fixupcontext(&mut context);

		Self {
			thunk: Box::new(thunk),
			time_limit: timeout,
			time_out: 0,
			// We must box the context so its address won't change if a collection
			// relocates the UntypedContinuation that contains it!
			pause_resume: context,
			stack: vec![0; STACK_SIZE_BYTES].into_boxed_slice(),
		}
	}
}

// The RefMut automagically prevents Send'ing or Sync'hronizing this thread-local instance.
pub struct CallStack<'a> (Option<RefMut<'a, Vec<UntypedContinuation>>>);

impl CallStack<'static> {
	/// Returns a guard that holds preemption disabled throughout its lifetime.
	///
	/// Note that a preemption signal is asserted upon releasing the guard.  **As such, calling
	/// this from the associated signal handler will cause the latter to be invoked endlessly!**
	pub fn lock() -> Result<Self, Error> {
		use std::mem::forget;

		// Prevent preemption before we run any RefCell code!
		forget(PreemptGuard::block()?);

		// Assert because we should never find ourselves lock()'ing during teardown.
		Ok(CallStack (Some(call_stack_handle().unwrap().borrow_mut())))
	}

	/// Similar to `lock()`, but merely assumes that preemption is already disabled.
	///
	/// This function is only safe to call when preemption is impossible (e.g. while inside a
	/// signal handler; misuse opens the underlying RefCell to concurrency violations.  Returns
	/// an error if invoked during thread teardown.
	pub unsafe fn preempt() -> Result<RefMut<'static, Vec<UntypedContinuation>>, AccessError> {
		Ok(call_stack_handle()?.borrow_mut())
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
		let empty = self.0.as_ref().unwrap().is_empty();
		self.0.take();

		if ! empty {
			PreemptGuard::unblock().unwrap();
		}
	}
}

fn call_stack_handle() -> Result<&'static RefCell<Vec<UntypedContinuation>>, AccessError> {
	use std::mem::transmute;

	thread_local! {
		static CALL_STACK: RefCell<Vec<UntypedContinuation>> = RefCell::new(vec![]);
	}

	CALL_STACK.try_with(|call_stack| unsafe {
		transmute(call_stack)
	})
}