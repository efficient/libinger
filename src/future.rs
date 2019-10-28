use linger::Linger;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::thread::Result as ThdResult;

pub struct PreemptiveFuture<T, F: FnMut(*mut Option<ThdResult<T>>) + Send> {
	fun: Option<Linger<T, F>>,
	us: u64,
}

pub fn poll_fn<T: Send>(mut fun: impl FnMut() -> Poll<T> + Send, us: u64)
-> PreemptiveFuture<T, impl FnMut(*mut Option<ThdResult<T>>) + Send> {
	use linger::launch;
	use linger::pause;
	use std::hint::unreachable_unchecked;

	let fun = Some(launch(move || {
		let mut res;
		while {
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
	}, 0).unwrap());
	PreemptiveFuture {
		fun,
		us,
	}
}

impl<T, F: FnMut(*mut Option<ThdResult<T>>) + Send> Future for PreemptiveFuture<T, F> {
	type Output = T;

	fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<Self::Output> {
		use linger::resume;

		if let Some(mut fun) = self.fun.take() {
			resume(&mut fun, self.us).unwrap();
			if let Linger::Completion(ready) = fun {
				Poll::Ready(ready)
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
		} else {
			Poll::Pending
		}
	}
}
