use id::Id;
use libc::SIGVTALRM;
use libc::sigset_t;
use libc::ucontext_t;
use stable::StableMutAddr;
use std::cell::RefCell;
use std::cell::RefMut;
use std::io::Error;
use std::io::Result;
use std::ops::Deref;
use std::ops::DerefMut;
use std::os::raw::c_int;
use std::result::Result as StdResult;
use swap::Swap;
use uninit::Uninit;
use void::Void;

const SIGSETCONTEXT: c_int = SIGVTALRM;

///! A continuation representing a "snapshot" of this thread's execution at a particular point in time.
///!
///! There are two types of continuations: a _normal_ continuation or a _call gate_ continuation.
///! The difference is that the former execute on the same execution stack, whereas the latter execute on a dedicated owned stack.
///! Passing the wrong type of continuation to a function results in a runtime error indicated by a sentinel return value, usually `None`.
///!
///! Each continuation has some dynamic lifetime during which its stack frame persists and is safe to execute code on.
///! These are tracked at runtime, and an attempt to perform a continuation action on an expired `Context` results in an error.
///! Even though they have their own execution stacks, call gate continuations have a finite lifetime because they return to the original stack upon completion.
///! Some call gates have a `StableMutAddr` type parameter rather than a mere `DerefMut`;
///! since their stacks outlive the creation stack frame such continuations may be transplanted onto a new successor stack frame to extend their lifetime.
pub struct Context<S: DerefMut<Target = [u8]> = Void> {
	id: Id,
	context: RefCell<ucontext_t>,
	persistent: Option<Persistent<S>>,
}

///! The context received by a signal handler as its third argument.
///!
///! See the `Context` struct's `swap()` method.
pub type HandlerContext = ucontext_t;

struct Persistent<S: DerefMut<Target = [u8]>> {
	stack: S,
	link: &'static mut *mut ucontext_t,
	successor: Id,
	handler: bool,
}

///! The signal mask that will be restored along with a continuation.
pub struct SignalMask<'a> (RefMut<'a, ucontext_t>);

///! Checkpoint the execution state of the thread.
///!
///! Calling this function results in a call to the `scope` closure, which may optionally call `setcontext()` on its continuation argument.
///! If and only if it does so, the `checkpoint` closure is called.
///!
///! Note that by storing the continuation, it is possible to invoke `setcontext()` again from within `checkpoint`, which is then restarted from the beginning.
///! However, the continuation is automatically invalidated once `getcontext()` returns, since its checkpointed stack frame no longer exists.
pub fn getcontext<T>(scope: impl FnOnce(Context<Void>) -> T, mut checkpoint: impl FnMut() -> T) -> Result<T> {
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

///! Run code on a separate execution stack.
///!
///! Call this function with an allocated region to use as a stack.
///! The `gate` function will be called immediately on the original stack, and may optionally `setcontext()` on its continuation argument.
///! If and only if it does so, the `call` closure is called on the dedicated stack.
///!
///! Note that by storing the continuation, it is possible to invoke `setcontext()` again from within `call`, which is then restarted from the beginning.
///! However, once `call` returns, control returns to the `makecontext()` call site and the continuation is automatically invalidated.
pub fn makecontext<S: DerefMut<Target = [u8]>>(stack: S, gate: impl FnOnce(Context<S>), call: fn()) -> Result<()> {
	use libc::pthread_sigmask;
	use std::mem::transmute;
	use std::ptr::null;
	use std::os::raw::c_uint;

	fn combine(lower: c_uint, upper: c_uint) -> usize {
		lower as usize | ((upper as usize) << 32)
	}

	extern "C" fn trampoline(gl: c_uint, gu: c_uint, sl: c_uint, su: c_uint) {
		let gate: fn() = unsafe {
			transmute(combine(gl, gu))
		};
		gate();

		let succ: *mut ucontext_t = combine(sl, su) as _;
		unsafe {
			pthread_sigmask(0, null(), &mut (*succ).uc_sigmask);
		}
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

				let successor: usize = context.uc_link as _;
				unsafe {
					makecontext(
						&mut *context,
						transmute(trampoline as extern "C" fn(c_uint, c_uint, c_uint, c_uint)),
						2,
						call,
						call >> 32,
						successor,
						successor >> 32,
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

///! Graft a continuation with an owned stack onto the current stack frame, restoring its validity.
///!
///! Normally, it is not possible to call a continuation after the stack frame where it was created no longer exists.
///! However, if the continuation is a call gate that owns its stack, client code may patch it by calling this function on it.
///! (This constraint is checked at compile time: note the tighter type bound.)
///!
///! Upon calling this function, the `scope` closure is invoked, which may then choose to invoke `setcontext()` or `sigsetcontext()` on the updated continuation, as appropriate.
///! Even if the continuation had previously been invalidated, a valid such call will now succeed, and control will return to the call site of `restorecontext()` upon completion.
///! However, once `restorecontext()` returns, the continuation is automatically invalidated once again.
pub fn restorecontext<S: StableMutAddr<Target = [u8]>>(mut persistent: Context<S>, scope: impl FnOnce(Context<S>)) -> StdResult<(), Option<Error>> {
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

///! Call a continuation produced by `getcontext()` or `makecontext()`.
///!
///! Upon success, this function never returns.
///! If it does return, the error value may be either of:
///! * `None`, to indicate that `continuation` was invalid (expired or obtained from a signal handler)
///! * `Some(Error)`, to indicate a platform error
///!
///! **Note that this function's atypical control flow makes it easy to leak memory.
///! To avoid this pitfall, be sure to `drop()` local variables _before_ calling it.**
#[must_use]
pub fn setcontext<S: DerefMut<Target = [u8]>>(continuation: *const Context<S>) -> Option<Error> {
	use errno::errno;
	use invar::MoveInvariant;
	use libc::pthread_sigmask;
	use libc::setcontext;
	use std::ptr::null;

	let erryes = *errno();
	let continuation = unsafe {
		continuation.as_ref()
	}?;
	if ! validatecontext(continuation, false) {
		None?;
	}

	let mut context = continuation.context.borrow_mut();
	context.after_move();
	unsafe {
		pthread_sigmask(0, null(), &mut context.uc_sigmask);
	}
	drop(context);
	*errno() = erryes;
	unsafe {
		setcontext(continuation.context.as_ptr());
	}

	Some(Error::last_os_error())
}

///! Restore a checkpoint that was saved from a signal handler using `Context::swap()`.
///!
///! Upon success, this function never returns.
///! If it does return, the error value has the same meaning as that of `setcontext()`.
///!
///! **Note that this function's atypical control flow makes it easy to leak memory.
///! To avoid this pitfall, be sure to `drop()` local variables _before_ calling it.**
#[must_use]
pub fn sigsetcontext<S: StableMutAddr<Target = [u8]>>(continuation: *mut Context<S>) -> Option<Error> {
	use errno::errno;
	use libc::pthread_kill;
	use libc::pthread_self;
	use std::cell::Cell;
	use std::mem::transmute;
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	static INIT: Once = ONCE_INIT;
	thread_local! {
		static CHECKPOINT: Cell<Option<(&'static mut dyn Swap<Other = HandlerContext>, c_int)>> = Cell::new(None);
	}

	let erryes = *errno();
	if ! validatecontext(unsafe {
		continuation.as_ref()
	}?, true) {
		*errno() = erryes;
		return setcontext(continuation);
	}

	let mut err = None;
	INIT.call_once(|| {
		use libc::SA_SIGINFO;
		use libc::sigaction;
		use libc::siginfo_t;
		use std::ptr::null_mut;
		use zero::Zero;

		extern "C" fn handler(_: c_int, _: Option<&siginfo_t>, context: Option<&mut HandlerContext>) {
			let (checkpoint, erryes) = CHECKPOINT.with(|checkpoint| checkpoint.take()).unwrap();
			let context = context.unwrap();
			let success = checkpoint.swap(context);
			debug_assert!(success);
			*errno() = erryes;
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

	let continuation: *mut dyn Swap<Other = HandlerContext> = continuation;
	CHECKPOINT.with(|checkpoint| checkpoint.set(Some((unsafe {
		transmute(continuation)
	}, erryes))));
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

	///! Obtain write access to this continuation's signal mask.
	pub fn mask(&mut self) -> SignalMask {
		SignalMask (self.context.borrow_mut())
	}
}

impl<S: StableMutAddr<Target = [u8]>> Swap for Context<S> {
	type Other = HandlerContext;

	///! Call from a signal handler to exchange that handler's context with this call gate.
	///!
	///! The continuation on which this is called must have been obtained from a call to `makecontext()`;
	///! otherwise, this method simply returns `false`.
	///! The immediate effect of a successful call is that, upon returning from the signal handler, control is transferred to this continuation's _successor_.
	///! Meanwhile, the handler's original context is saved in this continuation.
	///! Client code may later restore the checkpoint using the `sigsetcontext()` function.
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
		this.swap(other);

		true
	}
}

impl<'a> Deref for SignalMask<'a> {
	type Target = sigset_t;

	fn deref(&self) -> &Self::Target {
		let SignalMask (this) = self;
		&this.uc_sigmask
	}
}

impl<'a> DerefMut for SignalMask<'a> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		let SignalMask (this) = self;
		&mut this.uc_sigmask
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
