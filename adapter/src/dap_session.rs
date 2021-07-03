use crate::prelude::*;

use crate::debug_protocol::*;
use futures::prelude::*;
use std::collections::{hash_map::Entry, HashMap};
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use tokio::sync::{broadcast, mpsc, oneshot};

pub trait DAPChannel:
    Stream<Item = Result<ProtocolMessage, io::Error>> + Sink<ProtocolMessage, Error = io::Error> + Send
{
}

impl<T> DAPChannel for T where
    T: Stream<Item = Result<ProtocolMessage, io::Error>> + Sink<ProtocolMessage, Error = io::Error> + Send
{
}

#[derive(Clone)]
pub struct DAPSession {
    requests_sender: Weak<broadcast::Sender<Request>>,
    events_sender: Weak<broadcast::Sender<Event>>,
    out_sender: mpsc::Sender<(ProtocolMessage, Option<oneshot::Sender<ResponseBody>>)>,
}

impl DAPSession {
    pub fn new(channel: Box<dyn DAPChannel>) -> (DAPSession, impl Future<Output = ()> + Send) {
        let mut channel: Pin<Box<dyn DAPChannel>> = channel.into();
        let requests_sender = Arc::new(broadcast::channel::<Request>(100).0);
        let events_sender = Arc::new(broadcast::channel::<Event>(100).0);
        let (out_sender, mut out_receiver) = mpsc::channel(100);
        let mut pending_requests: HashMap<u32, oneshot::Sender<ResponseBody>> = HashMap::new();
        let mut message_seq = 0;

        let client = DAPSession {
            requests_sender: Arc::downgrade(&requests_sender),
            events_sender: Arc::downgrade(&events_sender),
            out_sender: out_sender,
        };

        let worker = async move {
            loop {
                tokio::select! {
                    maybe_message = channel.next() => {
                        match maybe_message {
                            Some(message) => match message {
                                Ok(ProtocolMessage::Request(request)) => log_errors!(requests_sender.send(request)),
                                Ok(ProtocolMessage::Event(event)) => log_errors!(events_sender.send(event)),
                                Ok(ProtocolMessage::Response(response)) => match pending_requests.entry(response.request_seq) {
                                    Entry::Vacant(_) => {
                                        error!("Received response without a pending request (request_seq={})", response.request_seq);
                                    }
                                    Entry::Occupied(entry) => {
                                        let sender = entry.remove();
                                        if let Some(body) = response.body {
                                            if let Err(_) = sender.send(body) {
                                                error!("Requestor is gone (request_seq={})", response.request_seq);
                                            }
                                        }
                                    }
                                },
                                Err(_) => break,
                            },
                            None => {
                                debug!("Client has disconnected");
                                break
                            }
                         }
                     },
                    Some((message, response_sender)) = out_receiver.recv() => {
                        let mut message = message;
                        match &mut message {
                            ProtocolMessage::Request(request) => {
                                message_seq += 1;
                                request.seq = message_seq;
                                if let Some(response_sender) = response_sender {
                                     pending_requests.insert(request.seq, response_sender);
                                }
                            },
                            ProtocolMessage::Event(event) => {
                                message_seq += 1;
                                event.seq = message_seq;
                            },
                            ProtocolMessage::Response(_) => {}
                        }
                        log_errors!(channel.send(message).await);
                    }
                }
            }
        };

        (client, worker)
    }

    pub fn subscribe_requests(&self) -> Result<broadcast::Receiver<Request>, Error> {
        match self.requests_sender.upgrade() {
            Some(r) => Ok(r.subscribe()),
            None => Err("Sender is gone".into()),
        }
    }

    #[allow(unused)]
    pub fn subscribe_events(&self) -> Result<broadcast::Receiver<Event>, Error> {
        match self.events_sender.upgrade() {
            Some(r) => Ok(r.subscribe()),
            None => Err("Sender is gone".into()),
        }
    }

    pub async fn send_request(&self, request_args: RequestArguments) -> Result<ResponseBody, Error> {
        let (sender, receiver) = oneshot::channel();
        let message = ProtocolMessage::Request(Request {
            command: Command::Known(request_args),
            seq: 0,
        });
        self.out_sender.send((message, Some(sender))).await?;
        Ok(receiver.await?)
    }

    #[allow(unused)]
    pub async fn send_response(&self, response: Response) -> Result<(), Error> {
        let message = ProtocolMessage::Response(response);
        self.out_sender.send((message, None)).await?;
        Ok(())
    }

    pub fn try_send_response(&self, response: Response) -> Result<(), Error> {
        let message = ProtocolMessage::Response(response);
        self.out_sender.try_send((message, None))?;
        Ok(())
    }

    pub async fn send_event(&self, event_body: EventBody) -> Result<(), Error> {
        let message = ProtocolMessage::Event(Event {
            body: event_body,
            seq: 0,
        });
        self.out_sender.send((message, None)).await?;
        Ok(())
    }

    pub fn try_send_event(&self, event_body: EventBody) -> Result<(), Error> {
        let message = ProtocolMessage::Event(Event {
            body: event_body,
            seq: 0,
        });
        self.out_sender.try_send((message, None))?;
        Ok(())
    }
}
