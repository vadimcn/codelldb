use crate::prelude::*;
use lldb::{SBEvent, SBListener};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{self, error::TrySendError};

pub struct DebugEventListener {
    state: Mutex<Inner>,
}

struct Inner {
    // Indicates whether events are currently corked.
    corked: bool,
    // A sticky copy of corked, which indicates whether corked was true at any point
    // while we were waiting on SBEventListener.
    was_corked: bool,
}

impl DebugEventListener {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(Inner {
                corked: false,
                was_corked: false,
            }),
        })
    }

    /// Start polling the SBListener and send events through a channel.
    /// Returns a receiver for the events.  Polling will stop when it is dropped.
    pub fn start_polling(
        self: &Arc<Self>,
        event_listener: &SBListener,
        channel_capacity: usize,
    ) -> mpsc::Receiver<SBEvent> {
        let mut event_listener = event_listener.clone();
        let (sender, receiver) = mpsc::channel(channel_capacity);

        let self_ref = self.clone();
        tokio::task::spawn(async move {
            let mut received;
            let mut event = SBEvent::new();
            loop {
                (received, event_listener, event) = tokio::task::spawn_blocking(move || {
                    let received = event_listener.wait_for_event(1, &mut event);
                    (received, event_listener, event)
                }).await.unwrap();

                let mut state = self_ref.state.lock().unwrap();
                if received && !state.was_corked {
                    match sender.try_send(event) {
                        Ok(_) => {}
                        Err(err) => match err {
                            TrySendError::Full(_) => error!("Event listener: Could not send event: {:?}", err),
                            TrySendError::Closed(_) => break,
                        },
                    }
                    event = SBEvent::new();
                }
                state.was_corked = state.corked;
            }
            debug!("Event listener: Shutting down.");
        });

        receiver
    }

    /// Corks the event listener.
    /// Events received between the calls to cork() and uncork() are dropped.
    pub fn cork(&self) {
        let mut state = self.state.lock().unwrap();
        state.corked = true;
        state.was_corked = true;
    }

    /// Uncorks the event listener.
    pub fn uncork(&self) {
        let mut state = self.state.lock().unwrap();
        state.corked = false;
    }
}
