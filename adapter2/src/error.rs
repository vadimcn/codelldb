use lldb;
use serde_json;
use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::option;

#[derive(Debug)]
pub enum Error {
    // Out fault
    Internal(String),
    // VSCode's fault
    Protocol(String),
    // User's fault
    UserError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Internal(s) => write!(f, "Internal debugger error: {}", s),
            Error::Protocol(s) => write!(f, "Debug protocol error: {}", s),
            Error::UserError(s) => write!(f, "{}", s),
        }
    }
}
impl StdError for Error {}

impl From<option::NoneError> for Error {
    fn from(_: option::NoneError) -> Self {
        Error::Internal("Expected Option::Some, found None".into())
    }
}
impl From<lldb::SBError> for Error {
    fn from(err: lldb::SBError) -> Self {
        Error::Internal(err.description().into())
    }
}
impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Internal(err.description().into())
    }
}
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Internal(err.description().into())
    }
}
impl From<std::num::ParseIntError> for Error {
    fn from(err: std::num::ParseIntError) -> Self {
        Error::Internal(err.to_string())
    }
}
impl From<python::Error> for Error {
    fn from(err: python::Error) -> Self {
        Error::Internal(err.to_string())
    }
}
