mod build_error;

use crate::build_error::Result;
use std::env::var;
use std::process::Command;

fn main() -> Result {
	let mut make = Command::new("make");
	make.arg("libgotcha.o");
	make.arg("libgotcha_api.rs");
	if var("DEBUG")?.parse()? {
		make.arg("ASFLAGS=-g");
		make.arg("CFLAGS=-g3 -Og");
		make.arg("RUSTFLAGS=-g -Copt-level=0");
	}
	make.status()?;

	let out_dir = var("OUT_DIR")?;
	let path = var("PATH")?;
	Command::new("cp")
		.arg("libgotcha.o")
		.arg(&out_dir)
		.status()
	?;
	Command::new("install")
		.arg("-m+rx")
		.arg("build_rustc")
		.arg(format!("{}/rustc", out_dir))
		.status()
	?;
	println!("cargo:rustc-env=PATH={}:{}", out_dir, path);

	Ok(())
}
