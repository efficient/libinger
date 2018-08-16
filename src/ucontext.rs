use invar::MoveInvariant;
use libc::sigset_t;
use libc::ucontext_t;
use std::cell::RefCell;
use std::io::Error;
use std::io::Result;
use std::rc::Rc;
use std::rc::Weak;
use uninit::Uninit;
use zero::Zero;

const REG_CSGSFS: usize = 18;

thread_local! {
	static GUARDS: RefCell<Vec<Rc<usize>>> = RefCell::new(Vec::new());
}

/// A continuation that may be resumed using `setcontext()`.
pub struct Context {
	context: RefCell<ucontext_t>,
	guard: Option<Weak<usize>>,
}

impl Context {
	/// NB: The returned object contains uninitialized data, and cannot be safely dropped until
	///     it has either been initialized or zeroed!
	fn new() -> Self {
		let mut context = ucontext_t::uninit();
		context.uc_mcontext.gregs = Zero::zero();

		Self {
			context: RefCell::new(context),
			guard: None,
		}

	}

	/// Exchange the functional portion of this context with another one.  When called on a
	/// a particular context within a signal handler, this causes that context to be restored
	/// upon return from the handler.  Note that the handler's original context is stored back
	/// unguarded, but that a subsequent `setcontext()`s is UB according to SUSv2.
	pub fn swap(&mut self, other: &mut ucontext_t) {
		use std::mem::swap;

		let mut this = self.context.borrow_mut();

		this.after_move();
		swap(&mut this.uc_mcontext, &mut other.uc_mcontext);
		swap(&mut this.uc_mcontext.gregs[REG_CSGSFS], &mut other.uc_mcontext.gregs[REG_CSGSFS]);

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

#[inline(always)]
fn checkpoint(context: &Context) -> Result<()> {
	use libc::getcontext;
	use std::ptr::write;

	if unsafe {
		getcontext(context.context.as_ptr())
	} != 0 {
		// Zero the uninitialized context before dropping it!
		unsafe {
			write(context.context.as_ptr(), ucontext_t::zero());
		}
		Err(Error::last_os_error())?;
	}

	Ok(())
}

/// Calls `a()`, which may perform a `setcontext()` on its argument.  If and only if it does so,
/// `b()` is executed before this function returns.
pub fn getcontext<T, A: FnOnce(Context) -> T, B: FnOnce() -> T>(a: A, b: B) -> Result<T> {
	use std::mem::forget;
	use volatile::VolBool;

	let mut context = Context::new();

	// Storing this flag on the stack is not unsound because guard enforces the invariant that
	// this stack frame outlives any resumable context.  Storing it on the stack is not leaky
	// because client code that never resumes the context was already responsible for cleaning
	// up this function's stack.
	let mut unused = VolBool::new(true);
	let guard = GUARDS.with(|guards| {
		let mut guards = guards.borrow_mut();
		let guard = Rc::new(guards.len());
		let res = Rc::downgrade(&guard);
		guards.push(guard);
		res
	});
	context.guard = Some(guard.clone());
	checkpoint(&context)?;

	let res;
	if unused.load() {
		unused.store(false);
		res = a(context);
	} else {
		forget(context);
		res = b();
	}

	GUARDS.with(move |guards| {
		guards.borrow_mut().truncate(*guard.upgrade().unwrap())
	});
	Ok(res)
}

/// Configures a context to invoke `function()` on a separate `stack`, optionally resuming the
/// program at the `successor` context upon said function's return (or, by default, exiting).
pub fn makecontext(function: extern "C" fn(), stack: &mut [u8], successor: Option<&mut Context>) -> Result<Context> {
	use libc::makecontext;

	let context = Context::new();
	checkpoint(&context)?;
	{
		let mut ucontext = context.context.borrow_mut();
		ucontext.uc_stack.ss_sp = stack.as_mut_ptr() as _;
		ucontext.uc_stack.ss_size = stack.len();
		if let Some(successor) = successor {
			ucontext.uc_link = successor.context.as_ptr();
		}

		unsafe {
			makecontext(&mut *ucontext, function, 0);
		}
	}
	Ok(context)
}

/// Attempts to resume `context`, never returning on success.  Otherwise, returns `None` if
/// `context`'s stack frame has expired or `Some` to indicate a platform error.
pub fn setcontext(context: &Context) -> Option<Error> {
	use libc::setcontext;

	if let Some(guard) = context.guard.as_ref() {
		let guard = guard.upgrade()?;
		GUARDS.with(|guards| guards.borrow_mut().truncate(*guard + 1));
	}

	let mut ucontext = context.context.borrow_mut();
	ucontext.after_move();
	unsafe {
		setcontext(&*ucontext);
	}
	Some(Error::last_os_error())
}

pub fn sigsetcontext(context: Context) -> Error {
	use libc::SA_SIGINFO;
	use libc::SIGVTALRM;
	use libc::pthread_kill;
	use libc::pthread_self;
	use libc::sigaction;
	use libc::siginfo_t;
	use std::cell::Cell;
	use std::os::raw::c_int;
	use std::ptr::null_mut;
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	static INIT: Once = ONCE_INIT;

	thread_local! {
		static CONTEXT: Cell<Option<Context>> = Cell::new(None);
	}

	INIT.call_once(|| {
		extern "C" fn handler(_: c_int, _: Option<&siginfo_t>, context: Option<&mut ucontext_t>) {
			let context = context.unwrap();
			let mut protext = CONTEXT.with(|protext| protext.take()).unwrap();
			protext.swap(context);
		}

		let config = sigaction {
			sa_flags: SA_SIGINFO,
			sa_sigaction: handler as _,
			sa_restorer: None,
			sa_mask: sigset_t::zero(),
		};
		if unsafe {
			sigaction(SIGVTALRM, &config, null_mut())
		} != 0 {
			panic!(Error::last_os_error());
		}
	});

	CONTEXT.with(|protext| protext.set(Some(context)));
	unsafe {
		pthread_kill(pthread_self(), SIGVTALRM);
	}
	Error::last_os_error()
}

unsafe impl Uninit for ucontext_t {}
unsafe impl Zero for sigset_t {}
unsafe impl Zero for ucontext_t {}
