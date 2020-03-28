use futures::prelude::*;

use log::*;
use std::collections::{hash_map::Entry, HashMap};
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};

use tokio::sync::{broadcast, mpsc, oneshot};

use crate::debug_protocol::*;
use crate::error::Error;
use crate::wire_protocol::Codec;

pub trait DAPChannel:
    Stream<Item = Result<ProtocolMessage, io::Error>> + Sink<ProtocolMessage, Error = io::Error> + Send
{
}

impl<T> DAPChannel for T where
    T: Stream<Item = Result<ProtocolMessage, io::Error>> + Sink<ProtocolMessage, Error = io::Error> + Send
{
}

pub struct DAPSession {
    requests_sender: Weak<broadcast::Sender<Request>>,
    events_sender: Weak<broadcast::Sender<Event>>,
    out_sender: mpsc::Sender<(ProtocolMessage, Option<oneshot::Sender<ResponseBody>>)>,
    message_seq: u32,
}

impl DAPSession {
    pub fn new(channel: Box<dyn DAPChannel>) -> (DAPSession, impl Future<Output = ()> + Send) {
        let mut channel: Pin<Box<dyn DAPChannel>> = channel.into();
        let requests_sender = Arc::new(broadcast::channel::<Request>(10).0);
        let events_sender = Arc::new(broadcast::channel::<Event>(10).0);
        let (out_sender, mut out_receiver) = mpsc::channel(100);
        let mut pending_requests: HashMap<u32, oneshot::Sender<ResponseBody>> = HashMap::new();

        let client = DAPSession {
            requests_sender: Arc::downgrade(&requests_sender),
            events_sender: Arc::downgrade(&events_sender),
            out_sender: out_sender,
            message_seq: 0,
        };

        let worker = async move {
            loop {
                tokio::select! {
                    maybe_message = channel.next() => {
                        match maybe_message {
                            Some(message) => match message {
                                Ok(ProtocolMessage::Request(request)) => log_send_err(requests_sender.send(request)),
                                Ok(ProtocolMessage::Event(event)) => log_send_err(events_sender.send(event)),
                                Ok(ProtocolMessage::Response(response)) => match pending_requests.entry(response.request_seq) {
                                    Entry::Vacant(_) => {
                                        debug!("Received response without a pending request (request_seq={})", response.request_seq);
                                    }
                                    Entry::Occupied(entry) => {
                                        let mut sender = entry.remove();
                                        if let Some(body) = response.body {
                                            log_send_err(sender.send(body));
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
                    Some((out_message, response_sender)) = out_receiver.next() => {
                        match &out_message {
                            ProtocolMessage::Request(request)=> {
                                pending_requests.insert(request.seq, response_sender.unwrap());
                            }
                            _ => {}
                        }
                        log_send_err(channel.send(out_message).await);
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

    pub fn subscribe_events(&self) -> Result<broadcast::Receiver<Event>, Error> {
        match self.events_sender.upgrade() {
            Some(r) => Ok(r.subscribe()),
            None => Err("Sender is gone".into()),
        }
    }

    pub async fn send_request(&mut self, request_args: RequestArguments) -> Result<ResponseBody, Error> {
        self.message_seq += 1;
        let (sender, receiver) = oneshot::channel();
        let message = ProtocolMessage::Request(Request {
            seq: self.message_seq,
            command: Command::Known(request_args),
        });
        self.out_sender.send((message, Some(sender))).await?;
        let resp = receiver.await?;
        Ok(resp)
    }

    pub fn send_request_only(&mut self, request_args: RequestArguments) -> Result<(), Error> {
        self.message_seq += 1;
        let (sender, receiver) = oneshot::channel();
        let message = ProtocolMessage::Request(Request {
            seq: self.message_seq,
            command: Command::Known(request_args),
        });
        self.out_sender.try_send((message, Some(sender)))?;
        Ok(())
    }

    pub fn send_response(&mut self, response: Response) -> Result<(), Error> {
        let message = ProtocolMessage::Response(response);
        self.out_sender.try_send((message, None))?;
        Ok(())
    }

    pub fn send_event(&mut self, event_body: EventBody) -> Result<(), Error> {
        self.message_seq += 1;
        let message = ProtocolMessage::Event(Event {
            seq: self.message_seq,
            body: event_body,
        });
        self.out_sender.try_send((message, None))?;
        Ok(())
    }
}

fn log_send_err<T, E: std::fmt::Debug>(result: Result<T, E>) {
    if let Err(err) = result {
        error!("Send error: {:?}", err);
    }
}
