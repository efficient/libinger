use invar::MoveInvariant;
use libc::ucontext_t;
use std::io::Error;
use std::io::Result;
use std::rc::Rc;
use uninit::Uninit;

/// A continuation that may be resumed using `setcontext()`.
pub struct Context {
	context: ucontext_t,
	guard: Rc<()>,
}

impl Context {
	fn new() -> Self {
		Self {
			context: ucontext_t::uninit(),
			guard: Rc::new(()),
		}

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
	let guard = Rc::downgrade(&context.guard);
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

/// Attempts to resume `context`, never returning on success.  Otherwise, returns `None` if
/// `context`'s stack frame has expired or `Some` to indicate a platform error.
pub fn setcontext(mut context: Context) -> Option<Error> {
	use libc::setcontext;

	let guarded = Rc::weak_count(&context.guard);
	if guarded == 0 {
		None?;
	}
	debug_assert!(guarded == 1, "setcontext() found multiple corresponding stack frames (?)");
	drop(context.guard);

	context.context.after_move();
	unsafe {
		setcontext(&context.context);
	}
	Some(Error::last_os_error())
}

unsafe impl Uninit for ucontext_t {}
