use crate::prelude::*;

use adapter_protocol::*;
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
    requests_sender: Weak<broadcast::Sender<(u32, RequestArguments)>>,
    events_sender: Weak<broadcast::Sender<EventBody>>,
    out_sender: mpsc::Sender<(ProtocolMessageType, Option<oneshot::Sender<ResponseResult>>)>,
}

impl DAPSession {
    pub fn new(channel: Box<dyn DAPChannel>) -> (DAPSession, impl Future<Output = ()> + Send) {
        let mut channel: Pin<Box<dyn DAPChannel>> = channel.into();
        let requests_sender = Arc::new(broadcast::channel::<(u32, RequestArguments)>(100).0);
        let events_sender = Arc::new(broadcast::channel::<EventBody>(100).0);
        let (out_sender, mut out_receiver) = mpsc::channel(100);
        let mut pending_requests: HashMap<u32, oneshot::Sender<ResponseResult>> = HashMap::new();
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
                            Some(Ok(message)) => {
                                match message.type_ {
                                    ProtocolMessageType::Request(request) => log_errors!(requests_sender.send((message.seq, request))),
                                    ProtocolMessageType::Event(event) => log_errors!(events_sender.send(event)),
                                    ProtocolMessageType::Response(response) => match pending_requests.entry(response.request_seq) {
                                        Entry::Vacant(_) => {
                                            error!("Received response without a pending request (request_seq={})", response.request_seq);
                                        }
                                        Entry::Occupied(entry) => {
                                            let sender = entry.remove();
                                            if let Err(_) = sender.send(response.result) {
                                                error!("Requestor is gone (request_seq={})", response.request_seq);
                                            }
                                        }
                                    },
                                }
                            },
                            Some(Err(_)) => {break},
                            None => {
                                debug!("Client has disconnected");
                                break
                            }
                        }
                    },
                    Some((message_type, response_sender)) = out_receiver.recv() => {
                        message_seq += 1;
                        let message = ProtocolMessage {
                            seq: message_seq,
                            type_: message_type
                        };
                        if let Some(response_sender) = response_sender {
                            pending_requests.insert(message.seq, response_sender);
                        }
                        log_errors!(channel.send(message).await);
                    }
                }
            }
        };

        (client, worker)
    }

    pub fn subscribe_requests(&self) -> Result<broadcast::Receiver<(u32, RequestArguments)>, Error> {
        match self.requests_sender.upgrade() {
            Some(r) => Ok(r.subscribe()),
            None => Err("Sender is gone".into()),
        }
    }

    #[allow(unused)]
    pub fn subscribe_events(&self) -> Result<broadcast::Receiver<EventBody>, Error> {
        match self.events_sender.upgrade() {
            Some(r) => Ok(r.subscribe()),
            None => Err("Sender is gone".into()),
        }
    }

    pub async fn send_request(&self, request_args: RequestArguments) -> Result<ResponseBody, Error> {
        let (sender, receiver) = oneshot::channel();
        let request = ProtocolMessageType::Request(request_args);
        self.out_sender.send((request, Some(sender))).await?;
        let result = receiver.await?;
        match result {
            ResponseResult::Success {
                body,
            } => Ok(body),
            ResponseResult::Error {
                message,
                ..
            } => Err(message.into()),
        }
    }

    #[allow(unused)]
    pub async fn send_response(&self, response: Response) -> Result<(), Error> {
        self.out_sender.send((ProtocolMessageType::Response(response), None)).await?;
        Ok(())
    }

    pub fn try_send_response(&self, response: Response) -> Result<(), Error> {
        self.out_sender.try_send((ProtocolMessageType::Response(response), None))?;
        Ok(())
    }

    pub async fn send_event(&self, event_body: EventBody) -> Result<(), Error> {
        self.out_sender.send((ProtocolMessageType::Event(event_body), None)).await?;
        Ok(())
    }

    pub fn try_send_event(&self, event_body: EventBody) -> Result<(), Error> {
        self.out_sender.try_send((ProtocolMessageType::Event(event_body), None))?;
        Ok(())
    }
}
