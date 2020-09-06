use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;

#[derive(Debug)]
struct Inner {
    flag: AtomicBool,
    receiver_count: AtomicU16,
}
#[derive(Debug)]
pub struct Sender(Arc<Inner>);

impl Sender {
    pub fn new() -> Self {
        Sender(Arc::new(Inner {
            flag: AtomicBool::new(false),
            receiver_count: AtomicU16::new(0),
        }))
    }

    pub fn subscribe(&self) -> Receiver {
        self.0.receiver_count.fetch_add(1, Ordering::Relaxed);
        Receiver(self.0.clone())
    }

    pub fn send(&self) {
        self.0.flag.store(true, Ordering::Release);
    }

    pub fn receiver_count(&self) -> usize {
        self.0.receiver_count.load(Ordering::Relaxed) as usize
    }
}

impl Clone for Sender {
    fn clone(&self) -> Self {
        Sender(self.0.clone())
    }
}

#[derive(Debug)]
pub struct Receiver(Arc<Inner>);

impl Receiver {
    pub fn is_cancelled(&self) -> bool {
        self.0.flag.load(Ordering::Acquire)
    }
}

impl Clone for Receiver {
    fn clone(&self) -> Self {
        self.0.receiver_count.fetch_add(1, Ordering::Relaxed);
        Receiver(self.0.clone())
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.0.receiver_count.fetch_sub(1, Ordering::Relaxed);
    }
}

pub fn dummy() -> Receiver {
    Sender::new().subscribe()
}


#[test]
fn refcounts() {
    let sender = Sender::new();
    assert_eq!(sender.receiver_count(), 0);

    let recv1 = sender.subscribe();
    assert_eq!(sender.receiver_count(), 1);

    let recv2 = sender.subscribe();
    assert_eq!(sender.receiver_count(), 2);

    drop(recv1);
    assert_eq!(sender.receiver_count(), 1);

    let sender2 = sender.clone();
    let recv3 = sender2.subscribe();
    assert_eq!(sender.receiver_count(), 2);

    drop(recv2);
    drop(recv3);
    assert_eq!(sender.receiver_count(), 0);
    assert_eq!(sender2.receiver_count(), 0);
}
