use crate::linger::Linger;

use std::future::Future;
use std::io::Result;
use std::pin::Pin;
use std::sync::mpsc::SyncSender;
use std::task::Context;
use std::task::Poll;
use std::thread::Result as ThdResult;

pub struct PreemptiveFuture<
	T,
	F: FnMut(*mut Option<ThdResult<T>>) + Send,
	P: Fn() -> I + Unpin,
	I: FnMut() + Unpin,
> {
	fun: Option<Linger<T, F>>,
	us: u64,
	poll: P,
	pre: SyncSender<I>,
}

pub fn poll_fn<T: Send>(fun: impl FnMut() -> Poll<T> + Send, us: u64)
-> Result<PreemptiveFuture<T, impl FnMut(*mut Option<ThdResult<T>>) + Send, impl Fn() -> fn(), fn()>> {
	fn nop() {}
	fn nope() -> fn() { nop }
	poll_fns(nope, fun, us)
}

pub fn poll_fns<T: Send, I: FnMut() + Send + Unpin>(
	poll: impl Fn() -> I + Unpin,
	mut fun: impl FnMut() -> Poll<T> + Send,
	us: u64,
) -> Result<PreemptiveFuture<T, impl FnMut(*mut Option<ThdResult<T>>) + Send, impl Fn() -> I, I>> {
	use crate::linger::launch;
	use crate::linger::pause;

	use std::hint::unreachable_unchecked;
	use std::sync::mpsc::sync_channel;

	let (pre, prep): (SyncSender<I>, _) = sync_channel(1);
	let fun = Some(launch(move || {
		let mut res;
		while {
			prep.recv().unwrap()();
			res = fun();
			res.is_pending()
		} {
			pause();
		}
		if let Poll::Ready(res) = res {
			res
		} else {
			unsafe {
				unreachable_unchecked()
			}
		}
	}, 0)?);
	Ok(PreemptiveFuture {
		fun,
		us,
		poll,
		pre,
	})
}

impl<T, F: FnMut(*mut Option<ThdResult<T>>) + Send, P: Fn() -> I + Unpin, I: FnMut() + Unpin>
Future for PreemptiveFuture<T, F, P, I> {
	type Output = Result<T>;

	fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<Self::Output> {
		use crate::linger::resume;

		if let Some(mut fun) = self.fun.take() {
			self.pre.send((self.poll)()).unwrap();
			if let Err(or) = resume(&mut fun, self.us) {
				Poll::Ready(Err(or))
			} else {
				if let Linger::Completion(ready) = fun {
					Poll::Ready(Ok(ready))
				} else {
					let timeout = ! fun.yielded();
					self.fun.replace(fun);
					if timeout {
						// The preemptible function timed out rather than blocking
						// on some other future, so it's already ready to run again.
						context.waker().wake_by_ref();
					}
					Poll::Pending
				}
			}
		} else {
			Poll::Pending
		}
	}
}
