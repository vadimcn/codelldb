use std::fmt;

#[derive(Debug)]
pub struct UserError(pub String);

impl std::error::Error for UserError {}

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn as_user_error<E: ToString>(err: E) -> UserError {
    UserError(err.to_string())
}

pub type Error = Box<dyn std::error::Error>;

macro_rules! bail(($err:expr) => (return Err(From::from($err))));

macro_rules! log_errors(($e:expr) => (if let Err(err) = $e { error!("{}", err); }));
