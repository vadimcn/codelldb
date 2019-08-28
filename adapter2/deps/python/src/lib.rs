#![feature(try_trait)]

use failure::Fail;
use lldb::*;

#[derive(Debug)]
pub enum PythonValue {
    SBValue(SBValue),
    Int(i64),
    Bool(bool),
    String(String),
    Object(String),
}

#[repr(transparent)]
#[derive(Fail, Debug)]
#[fail(display = "Python error: {:?}", _0)]
pub struct Error(pub failure::Error);

pub trait EventSink {
    fn display_html(&self, html: String, title: Option<String>, position: Option<i32>, reveal: bool);
}

pub trait PythonInterface {
    fn evaluate(&self, expr: &str, is_simple_expr: bool, context: &SBExecutionContext) -> Result<SBValue, String>;
    fn evaluate_as_bool(&self, expr: &str, is_simple_expr: bool, context: &SBExecutionContext) -> Result<bool, String>;
    fn modules_loaded(&self, modules: &mut dyn Iterator<Item = &SBModule>);
}

pub type Entry = fn() -> Result<(), Error>;

pub type NewSession = fn(
    interpreter: SBCommandInterpreter,
    event_sink: Box<dyn EventSink + Send>,
) -> Result<Box<dyn PythonInterface>, Error>;

#[cfg(any(feature = "python2", feature = "python3"))]
mod _impl {
    use super::*;
    use failure::format_err;

    impl From<std::io::Error> for Error {
        fn from(e: std::io::Error) -> Self {
            Error(e.into())
        }
    }

    impl From<std::option::NoneError> for Error {
        fn from(_: std::option::NoneError) -> Self {
            Error(format_err!("Expected Option::Some, found None"))
        }
    }

    impl From<cpython::PyErr> for Error {
        fn from(err: cpython::PyErr) -> Self {
            Error(format_err!("{:?}", err))
        }
    }
}
