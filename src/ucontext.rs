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

pub fn makecontext(function: extern "C" fn(), stack: &mut [u8], successor: Option<&mut Context>) -> Result<Context> {
	use libc::makecontext;

	let mut context = getcontext(|context| context, || unreachable!())?;
	context.guard.take();
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
