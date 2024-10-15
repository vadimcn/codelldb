use std::fmt;

pub type Error = Box<dyn std::error::Error>;

#[derive(Debug, Copy, Clone)]
pub enum Blame {
    Internal, // Cuased by CodeLLDB itself, should not be displayed to the user.
    User,     // Caused by user input, should be displayed to the user.
    Nobody,   // Expected error, should not be displayed to the user or logged as an error.
}

#[derive(Debug)]
pub struct BlamedError {
    pub blame: Blame,
    pub inner: Error,
}

impl BlamedError {
    pub fn assign_blame(self, blame: Blame) -> BlamedError {
        BlamedError {
            blame,
            inner: self.inner,
        }
    }
}

impl From<Error> for BlamedError {
    fn from(err: Error) -> BlamedError {
        match err.downcast::<BlamedError>() {
            Ok(blamed) => *blamed,
            Err(err) => BlamedError {
                blame: Blame::Internal,
                inner: err,
            },
        }
    }
}

impl fmt::Display for BlamedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "blame({:?}) {}", self.blame, self.inner)
    }
}

impl std::error::Error for BlamedError {
    fn cause(&self) -> Option<&dyn std::error::Error> {
        Some(&*self.inner)
    }
}

pub fn blame_user(err: Error) -> BlamedError {
    BlamedError {
        blame: Blame::User,
        inner: err,
    }
}

pub fn str_error(err_msg: impl ToString) -> Error {
    err_msg.to_string().into()
}

macro_rules! bail(($err:expr) => (return Err(From::from($err))));

macro_rules! log_errors(($e:expr) => (if let Err(err) = $e { log::error!("[{}] {}", line!(), err); }));
