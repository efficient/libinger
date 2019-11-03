#[macro_use]
extern crate bencher;
extern crate inger;

use bencher::Bencher;

benchmark_group![bench, resume];

fn resume(lo: &mut Bencher) {
	use inger::Linger;
	use inger::resume;

	let mut linger: Linger<_, fn(_)> = Linger::Completion(());
	lo.iter(||
		drop(resume(&mut linger, u64::max_value()))
	);
}

benchmark_main! {
	bench
}
