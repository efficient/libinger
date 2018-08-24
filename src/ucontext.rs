use id::Id;
use libc::ucontext_t;
use stable::StableMutAddr;
use std::cell::RefCell;
use std::io::Error;
use std::io::Result;
use std::ops::DerefMut;
use uninit::Uninit;
use void::Void;

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

pub fn restorecontext<S: StableMutAddr<Target = [u8]>, F: FnOnce(Context<S>)>(mut persistent: Context<S>, scope: F) -> Result<()> {
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
	)
	// The inner context's guard is invalidated as collateral damage upon return from this call.
}

#[must_use]
pub fn setcontext<S: DerefMut<Target = [u8]>>(continuation: *const Context<S>) -> Option<Error> {
	use invar::MoveInvariant;
	use libc::setcontext;

	let continuation = unsafe {
		continuation.as_ref()
	}?;

	if ! continuation.id.is_valid() {
		None?;
	}
	continuation.id.invalidate_subsequent();
	debug_assert!(
		continuation.persistent.as_ref().map(|persistent|
			persistent.successor.is_valid()
		).unwrap_or(true),
		"setcontext(): makecontext()-generated Context is valid but has an invalid successor!"
	);

	continuation.context.borrow_mut().after_move();
	unsafe {
		setcontext(continuation.context.as_ptr());
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
		}))
	}

	fn from(persistent: Option<Persistent<S>>) -> Self {
		Self {
			id: Id::new(),
			context: RefCell::new(ucontext_t::uninit()),
			persistent,
		}
	}

	#[must_use]
	pub fn swap(&mut self, other: &mut HandlerContext) -> bool {
		use swap::Swap;

		let persistent = self.persistent.as_mut().unwrap();
		if ! persistent.successor.is_valid() {
			return false;
		}
		// Blacklist any further use of setcontext() with the successor context.  We're
		// about to restore it for the last time when the current signal handler returns.
		persistent.successor.invalidate();
		debug_assert!(
			! self.id.is_valid(),
			"Context::swap(): invalidating successor's guard did not invalidate my own!"
		);

		let mut this = self.context.borrow_mut();
		let link = *persistent.link;
		this.swap(&mut unsafe {
			*link
		});

		let HandlerContext (other) = other;
		other.swap(&mut this);

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
		use ucontext::HandlerContext;
		use ucontext::makecontext;

		let mut st = [0u8; 1_024];
		let mut first = None;
		let mut ack = [0u8; 1_024];
		let mut second = None;
		makecontext(&mut st[..], |thing| first = Some(thing), || unreachable!()).unwrap();
		makecontext(&mut ack[..], |thing| second = Some(thing), || unreachable!()).unwrap();

		let mut first = first.unwrap();
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
		assert!(uc_inbounds(second.uc_link, &*first));
	}

	fn uc_inbounds(within: *const ucontext_t, context: *const ucontext_t) -> bool {
		within > context && within < unsafe {
			context.add(1)
		}
	}
}
