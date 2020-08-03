extern crate bindgen;
use bindgen::builder;
use std::env::var;

const HEADER: &str = "src/tcb.h";
const BINDINGS: &str = "tcb.rs";

fn main() -> Result<(), &'static str> {
	let out = var("OUT_DIR").map_err(|_| "var(\"OUT_DIR\")")?;
	let out = format!("{}/{}", out, BINDINGS);
	println!("cargo:rerun-if-changed={}", HEADER);
	builder()
		.header(HEADER)
		.generate().map_err(|_| "generate()")?
		.write_to_file(out).map_err(|_| "write_to_file()")?;

	Ok(())
}
