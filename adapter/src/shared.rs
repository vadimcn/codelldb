use std::ops::DerefMut;
use std::sync::Arc;
use tokio::sync::{Mutex, TryLockError};

pub struct Shared<T>(Arc<Mutex<T>>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T> Shared<T> {
    pub fn new(inner: T) -> Self {
        Self(Arc::new(Mutex::new(inner)))
    }

    pub async fn map<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut g = self.0.lock().await;
        f(g.deref_mut())
    }

    pub fn try_map<F, R>(&self, f: F) -> Result<R, TryLockError>
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut g = self.0.try_lock()?;
        Ok(f(g.deref_mut()))
    }
}
