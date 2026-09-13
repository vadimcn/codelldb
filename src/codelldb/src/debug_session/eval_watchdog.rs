use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Interrupts Python-based variable evaluation when it runs past a deadline.
pub struct EvalWatchdog {
    state: Arc<WatchdogState>,
    thread: Option<thread::JoinHandle<()>>,
}

struct WatchdogState {
    /// The request thread is currently inside a python call.
    in_eval: AtomicBool,
    /// Keeps track if guarded request has finished (all python requests finished).
    done: AtomicBool,
    /// The deadline was exceeded at least once.
    fired: AtomicBool,
}

/// Clears `in_eval` on drop that tells the watchdog all finished without a timeout.
/// `in_eval` set to true by `guard` in watchdog.
pub struct EvalGuard<'a> {
    state: &'a WatchdogState,
}

impl Drop for EvalGuard<'_> {
    fn drop(&mut self) {
        self.state.in_eval.store(false, Ordering::Release);
    }
}

impl EvalWatchdog {
    const RETRY_INTERVAL: Duration = Duration::from_millis(25);
    // Keep this at some bounded limit of interrupts per watchdog thread.
    const MAX_FIRES: u32 = 400;

    pub fn start(deadline: Instant, interrupt: impl Fn() + Send + 'static) -> EvalWatchdog {
        let state = Arc::new(WatchdogState {
            in_eval: AtomicBool::new(false),
            done: AtomicBool::new(false),
            fired: AtomicBool::new(false),
        });
        let wstate = state.clone();
        let thread = thread::spawn(move || {
            let mut fires = 0;
            loop {
                if wstate.done.load(Ordering::Acquire) {
                    break;
                }
                let now = Instant::now();
                if now < deadline {
                    thread::park_timeout(deadline - now);
                    continue;
                }
                if fires >= Self::MAX_FIRES {
                    break;
                }
                if wstate.in_eval.load(Ordering::Acquire) {
                    if fires == 0 {
                        log::warn!("Variable evaluation has timed out; interrupting the Python formatter.");
                    }
                    wstate.fired.store(true, Ordering::Release);
                    interrupt();
                    fires += 1;
                }
                thread::park_timeout(Self::RETRY_INTERVAL);
            }
        });
        EvalWatchdog {
            state,
            thread: Some(thread),
        }
    }

    /// Marks the current thread as being inside a formatter-invoking call.
    pub fn guard(&self) -> EvalGuard<'_> {
        self.state.in_eval.store(true, Ordering::Release);
        EvalGuard { state: &self.state }
    }

    /// Whether the deadline was exceeded (at least one interrupt was sent).
    pub fn fired(&self) -> bool {
        self.state.fired.load(Ordering::Acquire)
    }

    /// Stops the watchdog thread and reports whether it ever fired.
    pub fn disarm(mut self) -> bool {
        self.state.done.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
        self.fired()
    }
}

impl Drop for EvalWatchdog {
    fn drop(&mut self) {
        self.state.done.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}
