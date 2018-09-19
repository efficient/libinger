#![feature(test)]

extern crate libc;
extern crate test;
extern crate timetravel;

#[allow(dead_code)]
mod lifetimes;

use libc::MINSIGSTKSZ;
use libc::SIGSTKSZ;
use libc::siginfo_t;
use libc::ucontext_t;
use std::mem::uninitialized;
use std::os::raw::c_int;
use std::ptr::read_volatile;
use std::ptr::write_volatile;
use test::Bencher;
use timetravel::HandlerContext;

#[bench]
fn get_native(lo: &mut Bencher) {
	use libc::getcontext;

	lo.iter(|| unsafe {
		getcontext(&mut uninitialized());
	});
}

#[bench]
fn get_timetravel(lo: &mut Bencher) {
	use timetravel::getcontext;

	lo.iter(|| getcontext(|_| (), || ()));
}

fn get_helper<T, F: FnMut(ucontext_t) -> T>(mut fun: F) {
	use libc::getcontext;

	let mut initial = true;
	unsafe {
		let mut context = uninitialized();
		getcontext(&mut context);
		if read_volatile(&initial) {
			write_volatile(&mut initial, false);
			fun(context);
		}
	}
}

#[bench]
fn getset_native(lo: &mut Bencher) {
	use libc::setcontext;

	lo.iter(|| get_helper(|context| unsafe {
		setcontext(&context)
	}));
}

#[bench]
fn getset_timetravel(lo: &mut Bencher) {
	use timetravel::getcontext;
	use timetravel::setcontext;

	lo.iter(|| getcontext(|context| setcontext(&context), || None));
}

fn make_helper<T, F: FnMut(ucontext_t) -> T>(stack: &mut [u8], gated: extern "C" fn(), mut fun: F) {
	use libc::getcontext;
	use libc::makecontext;

	get_helper(|mut context| {
		let mut gate = unsafe {
			uninitialized()
		};
		unsafe {
			getcontext(&mut gate);
		}
		gate.uc_stack.ss_sp = stack.as_mut_ptr() as _;
		gate.uc_stack.ss_size = stack.len();
		gate.uc_link = &mut context;
		unsafe {
			makecontext(&mut gate, gated, 0);
		}
		fun(gate);
	});
}

#[bench]
fn make_native(lo: &mut Bencher) {
	extern "C" fn stub() {}

	lo.iter(|| make_helper(&mut [0u8; MINSIGSTKSZ][..], stub, |_| ()));
}

#[bench]
fn make_timetravel(lo: &mut Bencher) {
	use timetravel::makecontext;

	let mut stack = [0u8; MINSIGSTKSZ];
	lo.iter(|| makecontext(&mut stack[..], |_| (), || ()));
}

#[bench]
fn makeset_native(lo: &mut Bencher) {
	use libc::setcontext;

	extern "C" fn stub() {}

	lo.iter(|| make_helper(&mut [0u8; MINSIGSTKSZ][..], stub, |gate| unsafe {
		setcontext(&gate)
	}));
}

#[bench]
fn makeset_timetravel(lo: &mut Bencher) {
	use timetravel::makecontext;
	use timetravel::setcontext;

	let mut stack = [0u8; MINSIGSTKSZ];
	lo.iter(|| makecontext(&mut stack[..], |gate| panic!(setcontext(&gate)), || ()));
}

#[bench]
fn swapsig_fork(lo: &mut Bencher) {
	use libc::CPU_SET;
	use libc::CPU_ZERO;
	use libc::fork;
	use libc::pthread_self;
	use libc::pthread_setaffinity_np;
	use libc::sched_getcpu;
	use libc::waitpid;
	use std::mem::size_of_val;
	use std::process::exit;
	use std::ptr::null_mut;

	let mut cpus = unsafe {
		uninitialized()
	};
	unsafe {
		CPU_ZERO(&mut cpus);
		CPU_SET(sched_getcpu() as _, &mut cpus);
		pthread_setaffinity_np(pthread_self(), size_of_val(&cpus), &cpus);
	}
	lo.iter(|| {
		let pid = unsafe {
			fork()
		};
		if pid == 0 {
			exit(0);
		} else {
			unsafe {
				waitpid(pid, null_mut(), 0);
			}
		}
	});
}

// Pass None to reset the internal state at the start of a new test.
fn swapsig_helper<T: ContextRefMut>(handler: Option<extern "C" fn(c_int, Option<&mut siginfo_t>, Option<T>)>) -> impl FnMut() {
	use libc::SA_SIGINFO;
	use libc::SIGUSR1;
	use libc::SIGUSR2;
	use libc::pthread_kill;
	use libc::pthread_self;
	use libc::sigaction;
	use std::cell::RefCell;
	use std::collections::VecDeque;
	use std::mem::zeroed;
	use std::ptr::null_mut;

	const SIGNUMS: [c_int; 2] = [SIGUSR1, SIGUSR2];

	// This assumes the benchmarks are *not* run in parallel!
	thread_local! {
		static SIGNALS: RefCell<VecDeque<c_int>> = RefCell::new(SIGNUMS.iter().cloned().collect());
	}
	let signal = SIGNALS.with(|signals| signals.borrow_mut().pop_front());
	let mut setup = None;
	if let Some(handler) = handler {
		setup = Some(sigaction {
			sa_flags: SA_SIGINFO,
			sa_sigaction: handler as _,
			sa_restorer: None,
			sa_mask: unsafe {
				zeroed()
			},
		});
	} else {
		SIGNALS.with(|signals| {
			let mut signals = signals.borrow_mut();
			signals.clear();
			for signal in SIGNUMS.iter() {
				signals.push_back(*signal);
			}
		});
	}

	move || {
		let signal = signal.unwrap();

		if let Some(setup) = setup.take() {
			unsafe {
				sigaction(signal, &setup, null_mut());
			}
		}

		unsafe {
			pthread_kill(pthread_self(), signal);
		}
	}
}

#[bench]
fn swapsig_native(lo: &mut Bencher) {
	use libc::getcontext;
	use libc::setcontext;
	use lifetimes::unbound_mut;
	use timetravel::Swap;

	static mut CHECKPOINT: Option<&'static mut ucontext_t> = None;
	static mut GATE: Option<&'static mut ucontext_t> = None;
	static mut LO: Option<&'static mut Bencher> = None;

	extern "C" fn checkpoint(_: c_int, _: Option<&mut siginfo_t>, context: Option<&mut ucontext_t>) {
		let context = context.unwrap();
		unsafe {
			GATE.as_mut()
		}.unwrap().swap(context);

		let mut checkpoint: ucontext_t = **unsafe {
			CHECKPOINT.as_ref()
		}.unwrap();
		checkpoint.swap(context);
	}

	extern "C" fn restore(_: c_int, _: Option<&mut siginfo_t>, context: Option<&mut ucontext_t>) {
		unsafe {
			GATE.as_mut()
		}.unwrap().swap(context.unwrap());
	}

	extern "C" fn benchmark() {
		unsafe {
			LO.as_mut()
		}.unwrap().iter(swapsig_helper(Some(checkpoint)));
	}

	let reset: Option<Handler> = None;
	swapsig_helper(reset);

	let mut swapsig_helper = swapsig_helper(Some(restore));
	unsafe {
		LO = Some(unbound_mut(lo));
	}
	make_helper(&mut [0u8; SIGSTKSZ][..], benchmark, |mut gate| {
		unsafe {
			GATE = Some(unbound_mut(&mut gate));
		}
		get_helper(|mut checkpoint| unsafe {
			CHECKPOINT = Some(unbound_mut(&mut checkpoint));
			setcontext(&mut gate);
		});

		let mut checkpoint = unsafe {
			uninitialized()
		};
		unsafe {
			CHECKPOINT = Some(unbound_mut(&mut checkpoint));
			getcontext(&mut checkpoint);
		}
		swapsig_helper();
	});
}

#[bench]
fn swapsig_timetravel(lo: &mut Bencher) {
	use lifetimes::unbound_mut;
	use timetravel::Context;
	use timetravel::Swap;
	use timetravel::makecontext;
	use timetravel::restorecontext;
	use timetravel::setcontext;
	use timetravel::sigsetcontext;

	static mut CHECKPOINT: Option<Context<Box<[u8]>>> = None;
	static mut GOING: bool = true;
	static mut LO: Option<&'static mut Bencher> = None;

	extern "C" fn handler(_: c_int, _: Option<&mut siginfo_t>, context: Option<&mut HandlerContext>) {
		unsafe {
			CHECKPOINT.as_mut()
		}.unwrap().swap(context.unwrap());
	}

	let reset: Option<Handler> = None;
	swapsig_helper(reset);

	let stack: Box<[_]> = Box::new([0u8; SIGSTKSZ]);
	drop(makecontext(
		stack,
		|gate| {
			let gate = unsafe {
				CHECKPOINT.get_or_insert(gate)
			};
			unsafe {
				LO = Some(unbound_mut(lo));
			}
			panic!(setcontext(gate));
		},
		|| {
			unsafe {
				LO.as_mut()
			}.unwrap().iter(swapsig_helper(Some(handler)));
			unsafe {
				GOING = false;
			}
		},
	));

	while {
		drop(restorecontext(
			unsafe {
				CHECKPOINT.take()
			}.unwrap(),
			|checkpoint| {
				let checkpoint = unsafe {
					CHECKPOINT.get_or_insert(checkpoint)
				};
				panic!(sigsetcontext(checkpoint));
			},
		));

		unsafe {
			GOING
		}
	} {}
}

#[derive(Clone)]
#[derive(Copy)]
struct Latency {
	count: u32,
	duration: i64,
}

impl Latency {
	const NEW: Self = Self {
		count: 0,
		duration: 0,
	};

	fn now() -> i64 {
		use std::time::SystemTime;

		let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
		(now.as_secs() * 1_000_000_000 + now.subsec_nanos() as u64) as _
	}

	fn log_entry(&self) -> Latency {
		Self {
			count: self.count + 1,
			duration: self.duration - Self::now(),
		}
	}

	fn log_exit(&self) -> Latency {
		Self {
			count: self.count,
			duration: self.duration + Self::now(),
		}
	}

	fn log_interrupting(&self) -> bool {
		self.duration < 0
	}

	fn mean(&self) -> i64 {
		self.duration / self.count as i64
	}
}

#[bench]
fn cswitch_yield(lo: &mut Bencher) {
	use libc::CPU_SET;
	use libc::CPU_ZERO;
	use libc::pthread_self;
	use libc::pthread_setaffinity_np;
	use libc::sched_getcpu;
	use libc::sched_yield;
	use std::mem::size_of_val;
	use std::sync::atomic::ATOMIC_BOOL_INIT;
	use std::sync::atomic::AtomicBool;
	use std::sync::atomic::Ordering;
	use std::thread::spawn;

	static FINISHED: AtomicBool = ATOMIC_BOOL_INIT;
	static mut ONE_WAY: Latency = Latency::NEW;

	let mut cpus = unsafe {
		uninitialized()
	};
	unsafe {
		CPU_ZERO(&mut cpus);
		CPU_SET(sched_getcpu() as _, &mut cpus);
		pthread_setaffinity_np(pthread_self(), size_of_val(&cpus), &cpus);
	}

	let thread = spawn(|| while ! FINISHED.load(Ordering::Relaxed) {
		unsafe {
			if ONE_WAY.log_interrupting() {
				ONE_WAY = ONE_WAY.log_exit();
			}
			sched_yield();
		}
	});
	lo.iter(|| unsafe {
		if ! ONE_WAY.log_interrupting() {
			ONE_WAY = ONE_WAY.log_entry();
		}
		sched_yield();
	});
	FINISHED.store(true, Ordering::Relaxed);
	thread.join().unwrap();

	let spaces: String = (0..26).map(|_| ' ').collect();
	println!("{}one-way: {:11} ns/iter", spaces, unsafe {
		ONE_WAY.mean()
	});
}

#[bench]
fn cswitch_handler(lo: &mut Bencher) {
	use std::cell::Cell;

	thread_local! {
		static ONE_WAY: Cell<Latency> = Cell::new(Latency::NEW);
	}

	extern "C" fn handler(_: c_int, _: Option<&mut siginfo_t>, _: Option<&mut ucontext_t>) {
		ONE_WAY.with(|one_way| one_way.replace(one_way.get().log_exit()));
	}

	let reset: Option<Handler> = None;
	swapsig_helper(reset);

	let mut swapsig_helper = swapsig_helper(Some(handler));
	lo.iter(|| {
		ONE_WAY.with(|one_way| one_way.replace(one_way.get().log_entry()));
		swapsig_helper();
	});

	ONE_WAY.with(|one_way| {
		let spaces: String = (0..26).map(|_| ' ').collect();
		println!("{}one-way: {:11} ns/iter", spaces, one_way.get().mean());
	});
}

trait ContextRefMut {}
impl<'a> ContextRefMut for &'a mut HandlerContext {}
impl<'a> ContextRefMut for &'a mut ucontext_t {}

type Handler = extern "C" fn(c_int, Option<&mut siginfo_t>, Option<&mut ucontext_t>);
