use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

struct Inner {
    flag: AtomicBool,
    receiver_count: AtomicU16,
    callbacks: Mutex<Vec<Box<dyn Fn() + Send + Sync>>>,
}
pub struct Sender(Arc<Inner>);

impl Sender {
    pub fn new() -> Self {
        Sender(Arc::new(Inner {
            flag: AtomicBool::new(false),
            receiver_count: AtomicU16::new(0),
            callbacks: Mutex::new(Vec::new()),
        }))
    }

    // Create a new Receiver.
    pub fn subscribe(&self) -> Receiver {
        self.0.receiver_count.fetch_add(1, Ordering::Relaxed);
        Receiver(self.0.clone())
    }

    // Request cancellation.
    pub fn send(&self) {
        self.0.flag.store(true, Ordering::Release);
        let callbacks = self.0.callbacks.lock().unwrap();
        for callback in callbacks.iter() {
            callback();
        }
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

pub struct Receiver(Arc<Inner>);

impl Receiver {
    // Determine if the cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.flag.load(Ordering::Acquire)
    }
    // Add a callback to be called asynchonously when the cancellation is requested.
    // This object may outlive the Receiver through which it was added.
    pub fn add_callback<F: Fn() + Send + Sync + 'static>(&self, callback: F) {
        self.0.callbacks.lock().unwrap().push(Box::new(callback));
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
