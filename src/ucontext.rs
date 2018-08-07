use libc::ucontext_t;
use std::io::Error;
use std::io::Result;
use std::rc::Rc;
use uninit::Uninit;

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

pub fn getcontext<A: FnOnce(Context), B: FnMut()>(a: A, mut b: B) -> Result<()> {
	use libc::getcontext;
	use volatile::VolBool;

	let mut context = Context::new();

	// Storing this flag on the stack is not unsound because guard enforces the invariant that
	// this stack frame outlives any resumable context.  Storing it on the stack is not a leaky
	// because client code that never resumes the context was already responsible for cleaning
	// up this function's stack.
	let mut unused = VolBool::new(true);
	let guard = Rc::downgrade(&context.guard);
	if unsafe {
		getcontext(&mut context.context)
	} != 0 {
		Err(Error::last_os_error())?;
	}

	if unused.load() {
		unused.store(false);
		a(context);
	} else {
		b();
	}

	drop(guard);
	Ok(())
}

pub fn setcontext(context: Context) -> Option<Error> {
	use libc::setcontext;

	let guarded = Rc::weak_count(&context.guard);
	if guarded == 0 {
		None?;
	}
	debug_assert!(guarded == 1, "setcontext() found multiple corresponding stack frames (?)");

	drop(context.guard);
	unsafe {
		setcontext(&context.context);
	}
	Some(Error::last_os_error())
}

unsafe impl Uninit for ucontext_t {}
