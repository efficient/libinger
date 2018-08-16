use std::env::VarError;
use std::io::Error;
use std::process::ExitStatus;

const LIBS: [&str; 1] = [
	"libsp.a",
];

const SRCDIR: &str = "native";

pub fn main() -> Result<(), Failure> {
	use std::env::var;
	use std::process::Command;

	let destdir = format!("target/{}/deps", var("PROFILE")?);
	let srcdir = format!("arch/{}", var("CARGO_CFG_TARGET_ARCH")?);
	for lib in &LIBS {
		let filename = format!("{}/{}", srcdir, lib);
		Command::new("make").arg("-C").arg(SRCDIR).arg(&filename).status().success()?;
		let filename = format!("{}/{}", SRCDIR, filename);
		Command::new("mv").arg(&filename).arg(&destdir).status().success()?;
	}

	Ok(())
}

#[derive(Debug)]
pub enum Failure {
	VarError(VarError),
	Error(Error),
	ExitStatus(ExitStatus),
}

impl From<VarError> for Failure {
	fn from(ve: VarError) -> Self {
		Failure::VarError(ve)
	}
}

trait FailureResult {
	type FailRes;

	fn success(self) -> Self::FailRes;
}

impl FailureResult for Result<ExitStatus, Error> {
	type FailRes = Result<(), Failure>;

	fn success(self) -> Self::FailRes {
		match self {
			Ok(exit_status) =>
				if exit_status.success() {
					Ok(())
				} else {
					Err(Failure::ExitStatus(exit_status))
				},
			Err(error) => Err(Failure::Error(error)),
		}
	}
}
