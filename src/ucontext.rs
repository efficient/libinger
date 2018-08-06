use libc::ucontext_t;
use std::io::Error;
use std::io::Result;
use std::rc::Rc;
use uninit::Uninit;

pub struct Context {
	context: ucontext_t,
	ready: bool,
	guard: Rc<()>,
}

impl Context {
	fn new() -> Self {
		Self {
			context: ucontext_t::uninit(),
			ready: false,
			guard: Rc::new(()),
		}

	}
}

pub fn getcontext<A: FnOnce(Context), B: FnOnce()>(a: A, b: B) -> Result<()> {
	use libc::getcontext;

	let mut context = Context::new();
	let guard = Rc::downgrade(&context.guard);
	if unsafe {
		getcontext(&mut context.context)
	} != 0 {
		Err(Error::last_os_error())?;
	}

	if ! context.ready {
		context.ready = true;
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
	debug_assert!(guarded == 1);

	drop(context.guard);
	unsafe {
		setcontext(&context.context);
	}
	Some(Error::last_os_error())
}

unsafe impl Uninit for ucontext_t {}
