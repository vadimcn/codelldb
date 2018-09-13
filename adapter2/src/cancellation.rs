use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct CancellationToken {
    flag: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

pub struct CancellationSource {
    flag: Arc<AtomicBool>,
}

impl CancellationSource {
    pub fn new() -> Self {
        CancellationSource {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        CancellationToken {
            flag: self.flag.clone(),
        }
    }

    pub fn request_cancellation(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }
}
