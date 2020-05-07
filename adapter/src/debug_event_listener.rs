use futures::prelude::*;
use lldb::{IsValid, SBEvent, SBListener};
use log::{debug, error};
use tokio::sync::mpsc::{self, error::TrySendError};

pub fn start_polling(event_listener: &SBListener) -> impl Stream<Item = SBEvent> {
    let mut event_listener = event_listener.clone();
    let (mut sender, receiver) = mpsc::channel(1000);

    tokio::task::spawn(async move {
        let mut event = SBEvent::new();
        loop {
            let result = tokio::task::spawn_blocking(move || {
                event_listener.wait_for_event(1, &mut event);
                (event_listener, event)
            })
            .await
            .unwrap();

            event_listener = result.0;
            event = result.1;

            if event.is_valid() {
                match sender.try_send(event) {
                    Ok(_) => {}
                    Err(err) => match err {
                        TrySendError::Full(_) => error!("Event listener: Could not send event: {:?}", err),
                        TrySendError::Closed(_) => break,
                    },
                }
                event = SBEvent::new();
            }
        }
        debug!("Event listener: Shutting down.");
    });

    receiver
}
