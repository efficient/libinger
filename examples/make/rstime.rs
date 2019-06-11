#![crate_type = "dylib"]

extern crate gotcha;

enum TimeT {}

#[allow(unconditional_recursion)]
#[no_mangle]
extern fn time(tloc: Option<&mut TimeT>) {
	println!("time() from librstime");
	time(tloc)
}
