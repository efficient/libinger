use id::Id;
use libc::SIGVTALRM;
use libc::ucontext_t;
use stable::StableMutAddr;
use std::cell::RefCell;
use std::io::Error;
use std::io::Result;
use std::ops::DerefMut;
use std::os::raw::c_int;
use std::result::Result as StdResult;
use swap::Swap;
use uninit::Uninit;
use void::Void;

const SIGSETCONTEXT: c_int = SIGVTALRM;

pub struct Context<S: DerefMut<Target = [u8]>> {
	id: Id,
	context: RefCell<ucontext_t>,
	persistent: Option<Persistent<S>>,
}

pub struct HandlerContext (ucontext_t);

struct Persistent<S: DerefMut<Target = [u8]>> {
	stack: S,
	link: &'static mut *mut ucontext_t,
	successor: Id,
	handler: bool,
}

pub fn getcontext<T, A: FnOnce(Context<Void>) -> T, B: FnMut() -> T>(scope: A, mut checkpoint: B) -> Result<T> {
	use libc::getcontext;
	use std::mem::forget;
	use volatile::VolBool;

	let mut unused = VolBool::new(true);
	let this = Context::default();
	let guard = this.id;
	if unsafe {
		getcontext(this.context.as_ptr())
	} != 0 {
		Err(Error::last_os_error())?;
	}

	let res;
	if unused.read() {
		unused.write(false);
		res = scope(this);
	} else {
		forget(this);
		forget(scope);
		res = checkpoint();
	}

	guard.invalidate();
	drop(checkpoint);

	Ok(res)
}

pub fn makecontext<S: DerefMut<Target = [u8]>, F: FnOnce(Context<S>)>(stack: S, gate: F, call: fn()) -> Result<()> {
	use std::mem::transmute;
	use std::os::raw::c_uint;

	extern "C" fn trampoline(lower: c_uint, upper: c_uint) {
		let gate = lower as usize | ((upper as usize) << 32);
		let gate: fn() = unsafe {
			transmute(gate)
		};
		gate();
	}

	getcontext(
		|successor| -> Result<()> {
			use libc::getcontext;
			use libc::makecontext;
			use platform::Link;

			let mut this = Context::new(stack, successor.id);
			if unsafe {
				getcontext(this.context.as_ptr())
			} != 0 {
				Err(Error::last_os_error())?;
			}

			let call: usize = call as *const fn() as _;
			{
				let mut context = this.context.borrow_mut();
				let persistent = this.persistent.as_mut().unwrap();
				context.uc_stack.ss_sp = persistent.stack.as_mut_ptr() as _;
				context.uc_stack.ss_size = persistent.stack.len();
				context.uc_link = successor.context.as_ptr();
				unsafe {
					makecontext(
						&mut *context,
						transmute(trampoline as extern "C" fn(c_uint, c_uint)),
						2,
						call,
						call >> 32
					);
				}

				let link = context.link();
				debug_assert!(
					context.uc_link == *link,
					"makecontext(): inconsistent link address! (stack moved?)"
				);
				persistent.link = link;
			}
			gate(this);

			Ok(())
		},
		|| Ok(()),
	)??;
	// The inner context's guard is invalidated as collateral damage upon return from this call.

	Ok(())
}

pub fn restorecontext<S: StableMutAddr<Target = [u8]>, F: FnOnce(Context<S>)>(mut persistent: Context<S>, scope: F) -> StdResult<(), Option<Error>> {
	use platform::Stack;

	// Allow use on contexts from swap(), but not those from sigsetcontext(); the latter never
	// returns successfully, so we don't intend to allow using it as a checkpoint.
	let stack_ptr = persistent.context.borrow().stack_ptr();
	{
		let stack = &*persistent.persistent.as_ref().unwrap().stack;
		let stack_base = stack as *const _ as *const u8 as _;
		if stack_ptr < stack_base || stack_ptr > stack_base + stack.len() {
			Err(None)?;
		}
	}

	getcontext(
		|successor| {
			{
				let next = persistent.persistent.as_mut().unwrap();
				*next.link = successor.context.as_ptr();
				next.successor = successor.id;
			}
			persistent.id = Id::new();

			scope(persistent);
		},
		|| (),
	).map_err(|or| Some(or))
	// The inner context's guard is invalidated as collateral damage upon return from this call.
}

fn validatecontext<S: DerefMut<Target = [u8]>>(continuation: &Context<S>, handler_desired: bool) -> bool {
	if ! continuation.id.is_valid() {
		return false;
	}
	continuation.id.invalidate_subsequent();

	let handler;
	if let Some(persistent) = continuation.persistent.as_ref() {
		debug_assert!(
			persistent.successor.is_valid(),
			"setcontext(): makecontext()-generated Context is valid but has an invalid successor!"
		);
		handler = persistent.handler;
	} else {
		handler = false;
	}

	handler == handler_desired
}

#[must_use]
pub fn setcontext<S: DerefMut<Target = [u8]>>(continuation: *const Context<S>) -> Option<Error> {
	use invar::MoveInvariant;
	use libc::setcontext;

	let continuation = unsafe {
		continuation.as_ref()
	}?;

	if ! validatecontext(continuation, false) {
		None?;
	}

	continuation.context.borrow_mut().after_move();
	unsafe {
		setcontext(continuation.context.as_ptr());
	}

	Some(Error::last_os_error())
}

#[must_use]
pub fn sigsetcontext<S: StableMutAddr<Target = [u8]>>(continuation: *mut Context<S>) -> Option<Error> {
	use libc::pthread_kill;
	use libc::pthread_self;
	use std::cell::Cell;
	use std::mem::transmute;
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	static INIT: Once = ONCE_INIT;
	thread_local! {
		static CHECKPOINT: Cell<Option<*mut dyn Swap<Other = HandlerContext>>> = Cell::new(None);
	}

	if ! validatecontext(unsafe {
		continuation.as_ref()
	}?, true) {
		None?;
	}

	let mut err = None;
	INIT.call_once(|| {
		use libc::SA_SIGINFO;
		use libc::sigaction;
		use libc::siginfo_t;
		use std::ptr::null_mut;
		use zero::Zero;

		extern "C" fn handler(_: c_int, _: Option<&siginfo_t>, context: Option<&mut HandlerContext>) {
			let checkpoint = CHECKPOINT.with(|checkpoint| checkpoint.take()).unwrap();
			let checkpoint = unsafe {
				checkpoint.as_mut()
			}.unwrap();
			debug_assert!(checkpoint.swap(context.unwrap()));
		}

		let config = sigaction {
			sa_flags: SA_SIGINFO,
			sa_sigaction: handler as _,
			sa_restorer: None,
			sa_mask: Zero::zero(),
		};
		if unsafe {
			sigaction(SIGSETCONTEXT, &config, null_mut())
		} != 0 {
			err = Some(Error::last_os_error());
		}
	});
	if let Some(err) = err {
		return Some(err);
	}

	let continuation: *mut dyn Swap<Other = HandlerContext> = continuation as _;
	CHECKPOINT.with(|checkpoint| checkpoint.set(Some(unsafe {
		transmute(continuation)
	})));
	unsafe {
		pthread_kill(pthread_self(), SIGSETCONTEXT);
	}

	Some(Error::last_os_error())
}

impl Context<Void> {
	fn default() -> Self {
		Self::from(None)
	}
}

impl<S: DerefMut<Target = [u8]>> Context<S> {
	fn new(stack: S, successor: Id) -> Self {
		use std::mem::transmute;

		let link = unsafe {
			transmute(stack.as_ptr())
		};
		Self::from(Some(Persistent {
			stack,
			link,
			successor,
			handler: false,
		}))
	}

	fn from(persistent: Option<Persistent<S>>) -> Self {
		Self {
			id: Id::new(),
			context: RefCell::new(ucontext_t::uninit()),
			persistent,
		}
	}
}

impl<S: StableMutAddr<Target = [u8]>> Swap for Context<S> {
	type Other = HandlerContext;

	#[must_use]
	fn swap(&mut self, other: &mut Self::Other) -> bool {
		let persistent = self.persistent.as_mut().unwrap();

		let dest;
		if persistent.handler {
			// We're under a call to sigsetcontext(), whose input was already validated.
			dest = self.id;
		} else {
			// This is a direct call from client code; we must validate it.
			if ! persistent.successor.is_valid() {
				return false;
			}

			dest = persistent.successor;
		}

		// Blacklist any further use of setcontext() with the destination context.  We're
		// about to restore it for the last time when the current signal handler returns.
		dest.invalidate();
		debug_assert!(
			! self.id.is_valid(),
			"Context::swap(): invalidating destination guard did not invalidate my own!"
		);

		// What was originally the call gate will become a checkpoint of the current
		// preemption point.  Flag it to ensure we restore it using sigsetcontext().
		persistent.handler = ! persistent.handler;

		let mut this = self.context.borrow_mut();
		if persistent.handler {
			// If we were invoked directly by client code, perform a three-way swap so
			// that we restore the successor context rather than the call gate.
			let link = *persistent.link;
			this.swap(unsafe {
				&mut *link
			});
		}

		// We mustn't call the member function on the signal HandlerContext because this
		// will enforce the MoveInvariant, which is *not* correct for such contexts.  So
		// it's important that we flip the call order around.
		let HandlerContext (ref mut other) = other;
		this.swap(other);

		true
	}
}

#[cfg(test)]
mod tests {
	use libc::ucontext_t;

	#[test]
	fn context_moveinvariant() {
		use invar::MoveInvariant;
		use super::getcontext;

		let context = getcontext(|context| context, || unreachable!()).unwrap();
		let mut context = context.context.borrow_mut();
		assert!(! uc_inbounds(context.uc_mcontext.fpregs as _, &*context));
		context.after_move();
		assert!(uc_inbounds(context.uc_mcontext.fpregs as _, &*context));
	}

	#[test]
	fn context_swapinvariant() {
		use invar::MoveInvariant;
		use swap::Swap;
		use ucontext::HandlerContext;
		use ucontext::makecontext;

		let st: Box<[u8]> = Box::new([0u8; 1_024]);
		makecontext(st, |mut first| {
			let mut ack = [0u8; 1_024];
			let mut second = None;
			makecontext(&mut ack[..], |thing| second = Some(thing), || unreachable!()).unwrap();

			let second = second.unwrap();
			let mut second = HandlerContext (second.context.into_inner());
			{
				let mut first = first.context.borrow_mut();
				let HandlerContext (second) = &mut second;
				assert!(! uc_inbounds(first.uc_mcontext.fpregs as _, &*first));
				assert!(! uc_inbounds(second.uc_mcontext.fpregs as _, second));

				first.after_move();
				second.after_move();
				first.uc_link = first.uc_mcontext.fpregs as _;
				second.uc_link = second.uc_mcontext.fpregs as _;
				assert!(uc_inbounds(first.uc_link, &*first));
				assert!(uc_inbounds(second.uc_link, second));
			}
			assert!(first.swap(&mut second));

			let first = first.context.borrow();
			let HandlerContext (second) = &mut second;
			assert!(uc_inbounds(first.uc_mcontext.fpregs as _, &*first));
			assert!(uc_inbounds(second.uc_mcontext.fpregs as _, second));
			assert!(uc_inbounds(first.uc_link, second));
		}, || unreachable!()).unwrap();
	}

	fn uc_inbounds(within: *const ucontext_t, context: *const ucontext_t) -> bool {
		within > context && within < unsafe {
			context.add(1)
		}
	}
}
