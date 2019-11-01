use std::env::VarError;
use std::io::Error as IoError;
use std::result::Result as StdResult;
use std::str::ParseBoolError;

pub type Result = StdResult<(), Error>;

#[derive(Debug)]
pub enum Error {
	IoError(IoError),
	ParseError(ParseBoolError),
	VarError(VarError),
}

impl From<IoError> for Error {
	fn from(io_error: IoError) -> Self {
		Error::IoError(io_error)
	}
}

impl From<ParseBoolError> for Error {
	fn from(parse_error: ParseBoolError) -> Self {
		Error::ParseError(parse_error)
	}
}

impl From<VarError> for Error {
	fn from(var_error: VarError) -> Self {
		Error::VarError(var_error)
	}
}
