use invar::MoveInvariant;
use libc::ucontext_t;
use std::io::Error;
use std::io::Result;
use std::rc::Rc;
use uninit::Uninit;

/// A continuation that may be resumed using `setcontext()`.
pub struct Context {
	context: ucontext_t,
	guard: Option<Rc<()>>,
}

impl Context {
	fn new() -> Self {
		Self {
			context: ucontext_t::uninit(),
			guard: None,
		}

	}

	fn guard(&mut self) -> &Rc<()> {
		self.guard.get_or_insert_with(|| Rc::new(()))
	}

	/// Exchange the functional portion of this context with another one.  When called on a
	/// a particular context within a signal handler, this causes that context to be restored
	/// upon return from the handler.  Note that the handler's original context is stored back
	/// unguarded, but that a subsequent `setcontext()`s is UB according to SUSv2.
	pub fn swap(&mut self, other: &mut ucontext_t) {
		use std::mem::swap;

		let this = &mut self.context;

		this.after_move();
		swap(&mut this.uc_mcontext, &mut other.uc_mcontext);
		let this_fp = unsafe {
			this.uc_mcontext.fpregs.as_mut().unwrap()
		};
		let other_fp = unsafe {
			other.uc_mcontext.fpregs.as_mut().unwrap()
		};
		swap(this_fp, other_fp);
		swap(&mut this.uc_mcontext.fpregs, &mut other.uc_mcontext.fpregs);

		swap(&mut this.uc_flags, &mut other.uc_flags);
		swap(&mut this.uc_link, &mut other.uc_link);
		swap(&mut this.uc_stack, &mut other.uc_stack);
		swap(&mut this.uc_sigmask, &mut other.uc_sigmask);

		self.guard.take();
	}
}

/// Calls `a()`, which may perform a `setcontext()` on its argument.  If and only if it does so,
/// `b()` is executed before this function returns.
pub fn getcontext<T, A: FnOnce(Context) -> T, B: FnOnce() -> T>(a: A, b: B) -> Result<T> {
	use libc::getcontext;
	use volatile::VolBool;

	let mut context = Context::new();

	// Storing this flag on the stack is not unsound because guard enforces the invariant that
	// this stack frame outlives any resumable context.  Storing it on the stack is not leaky
	// because client code that never resumes the context was already responsible for cleaning
	// up this function's stack.
	let mut unused = VolBool::new(true);
	let guard = Rc::downgrade(context.guard());
	if unsafe {
		getcontext(&mut context.context)
	} != 0 {
		Err(Error::last_os_error())?;
	}

	let res;
	if unused.load() {
		unused.store(false);
		res = a(context);
	} else {
		res = b();
	}

	drop(guard);
	Ok(res)
}

/// Configures a context to invoke `function()` on a separate `stack`, optionally resuming the
/// program at the `successor` context upon said function's return (or, by default, exiting).
pub fn makecontext(function: extern "C" fn(), stack: &mut [u8], successor: Option<&mut Context>) -> Result<Context> {
	use libc::getcontext;
	use libc::makecontext;

	let mut context = Context::new();
	if unsafe {
		getcontext(&mut context.context)
	} != 0 {
		Err(Error::last_os_error())?;
	}

	context.context.uc_stack.ss_sp = stack.as_mut_ptr() as _;
	context.context.uc_stack.ss_size = stack.len();
	if let Some(successor) = successor {
		context.context.uc_link = &mut successor.context;
	}

	unsafe {
		makecontext(&mut context.context, function, 0);
	}
	Ok(context)
}

/// Attempts to resume `context`, never returning on success.  Otherwise, returns `None` if
/// `context`'s stack frame has expired or `Some` to indicate a platform error.
pub fn setcontext(mut context: Context) -> Option<Error> {
	use libc::setcontext;

	if let Some(guard) = context.guard.take() {
		let guarded = Rc::weak_count(&guard);
		if guarded == 0 {
			None?;
		}
		debug_assert!(guarded == 1, "setcontext() found multiple corresponding stack frames (?)");
	}

	context.context.after_move();
	unsafe {
		setcontext(&context.context);
	}
	Some(Error::last_os_error())
}

unsafe impl Uninit for ucontext_t {}
