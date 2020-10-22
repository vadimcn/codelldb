use ops::{Deref, DerefMut};
use std::ops;

#[derive(Debug)]
pub enum MustInitialize<T> {
    Initialized(T),
    NotInitialized,
}

pub use self::MustInitialize::Initialized;
pub use self::MustInitialize::NotInitialized;

impl<T> MustInitialize<T> {
    pub fn is_initialized(&self) -> bool {
        match self {
            Initialized(_) => true,
            NotInitialized => false,
        }
    }
    pub fn unwrap(&self) -> &T {
        self.deref()
    }
}

impl<T> Deref for MustInitialize<T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            Initialized(ref r) => r,
            NotInitialized => {
                panic!("Whoops! Something that was supposed to have been initialized at this point, wasn't.")
            }
        }
    }
}

impl<T> DerefMut for MustInitialize<T> {
    fn deref_mut(&mut self) -> &mut T {
        match self {
            Initialized(ref mut r) => r,
            NotInitialized => {
                panic!("Whoops! Something that was supposed to have been initialized at this point, wasn't.")
            }
        }
    }
}
