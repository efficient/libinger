use guard::PreemptGuard;
use libc::ucontext_t;
use pthread::Signal;
use pthread::pthread_kill;
use pthread::pthread_self;
use std::cell::Cell;
use std::cell::BorrowMutError;
use std::cell::RefCell;
use std::cell::RefMut;
use std::io::Error;
use std::mem::forget;
use std::rc::Rc;
use std::thread::AccessError;
use std::thread::panicking;
use zeroable::Zeroable;

const STACK_SIZE_BYTES: usize = 2 * 1_024 * 1_024;

pub struct UntypedContinuation {
	pub thunk: Option<Box<FnMut()>>,
	pub time_limit: u64,
	pub time_out: u64,
	pub pause_resume: Rc<RefCell<ucontext_t>>,
	pub stack: Box<[u8]>,
}

impl UntypedContinuation {
	pub fn new(timeout: u64) -> Self {
		Self {
			thunk: None,
			time_limit: timeout,
			time_out: 0,
			pause_resume: Rc::new(RefCell::new(ucontext_t::new())),
			stack: vec![0; STACK_SIZE_BYTES].into_boxed_slice(),
		}
	}
}

#[must_use]
pub struct CallStack<'a> {
	stack: &'a RefCell<Vec<UntypedContinuation>>,
	blocking: bool,
	deferred: &'a Cell<bool>,
}

impl CallStack<'static> {
	pub fn handle() -> Result<Self, AccessError> {
		use std::mem::transmute;

		thread_local! {
			static CALL_STACK: RefCell<Vec<UntypedContinuation>> = RefCell::new(vec![]);
			static DEFERRED: Cell<bool> = Cell::new(false);
		}

		Ok(Self {
			stack: CALL_STACK.try_with(|call_stack| unsafe {
				transmute(call_stack)
			})?,
			blocking: false,
			deferred: DEFERRED.try_with(|deferred| unsafe {
				transmute(deferred)
			})?,
		})
	}
}

impl<'a> CallStack<'a> {
	pub fn lock(&mut self) -> Result<RefMut<'a, Vec<UntypedContinuation>>, Error> {
		forget(PreemptGuard::block()?);
		self.blocking = true;
		Ok(self.stack.borrow_mut())
	}

	pub fn preempt(&self) -> Result<RefMut<'a, Vec<UntypedContinuation>>, BorrowMutError> {
		let call_stack = self.stack.try_borrow_mut();
		self.deferred.set(call_stack.is_err());
		call_stack
	}
}

impl<'a> Drop for CallStack<'a> {
	fn drop(&mut self) {
		if self.blocking && ! panicking() {
			if self.deferred.get() {
				pthread_kill(pthread_self(), Signal::Alarm).unwrap();
			}

			// If we're in the midst of panicking, we skip this line so that preemptions
			// remain disabled until we've unwound into resume(), which will
			// automatically reenable them as it drops its call stack handle.
			PreemptGuard::unblock().unwrap();
		}
	}
}
