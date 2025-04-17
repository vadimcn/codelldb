use crate::prelude::*;

use crate::dap_codec::{DecoderError, DecoderResult};

use adapter_protocol::*;
use futures::prelude::*;
use std::collections::{hash_map::Entry, HashMap};
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use tokio::sync::{broadcast, mpsc, oneshot};

pub trait DAPChannel:
    Stream<Item = Result<DecoderResult, io::Error>> + Sink<ProtocolMessage, Error = io::Error> + Send
{
}

impl<T> DAPChannel for T where
    T: Stream<Item = Result<DecoderResult, io::Error>> + Sink<ProtocolMessage, Error = io::Error> + Send
{
}

/// DAPSession implements communication between DAP message handlers and the DAP client (e.g. VSCode)
/// through a communication channel.
#[derive(Clone)]
pub struct DAPSession {
    // Used to communicate outgoing messages to the dispatcher.
    out_sender: Arc<mpsc::Sender<(ProtocolMessageType, Option<oneshot::Sender<ResponseResult>>)>>,
    // Used to create new subscriptions for requests and events.
    requests_sender: Weak<broadcast::Sender<(u32, RequestArguments)>>,
    events_sender: Weak<broadcast::Sender<EventBody>>,
}

impl DAPSession {
    /// Returns a new DAPSession object and a future representing the main dispatcher loop that
    /// routes messages between the DAP channel and the subscribers.
    /// The future will resolve when the DAP client closes the connection from its end.
    pub fn new(channel: Box<dyn DAPChannel>) -> (DAPSession, impl Future<Output = ()> + Send) {
        let mut channel: Pin<Box<dyn DAPChannel>> = channel.into();
        let requests_sender = Arc::new(broadcast::channel::<(u32, RequestArguments)>(100).0);
        let events_sender = Arc::new(broadcast::channel::<EventBody>(100).0);
        let (out_sender, mut out_receiver) = mpsc::channel(1000);
        let mut pending_requests: HashMap<u32, oneshot::Sender<ResponseResult>> = HashMap::new();
        let mut message_seq = 0;

        let client = DAPSession {
            requests_sender: Arc::downgrade(&requests_sender),
            events_sender: Arc::downgrade(&events_sender),
            out_sender: Arc::new(out_sender),
        };

        let dispatcher = async move {
            loop {
                tokio::select! {
                    maybe_result = channel.next() => {
                        match maybe_result {
                            Some(Ok(decoder_result)) => {
                                match decoder_result {
                                    Ok(message) => match message.type_ {
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
                                    Err(err) => match err {
                                        DecoderError::SerdeError { error, value } => {
                                            // The decoder read a complete frame, but failed to deserialize it
                                            error!("Deserialization error: {}", error);

                                            // Try to extract request seq
                                            use serde_json::value::*;
                                            let request_seq = match value {
                                                Value::Object(obj) => {
                                                    match obj.get("seq") {
                                                        Some(Value::Number(seq)) => seq.as_u64(),
                                                        _ => None,
                                                    }
                                                },
                                                _ => None
                                            };
                                            // If succeeded, send error response
                                            if let Some(request_seq) = request_seq {
                                                message_seq += 1;
                                                let message = ProtocolMessage {
                                                    seq: message_seq,
                                                    type_: ProtocolMessageType::Response(
                                                        Response {
                                                            request_seq: request_seq as u32,
                                                            success: false,
                                                            result: ResponseResult::Error {
                                                                message: "Malformed message".into(),
                                                                command: "".into(),
                                                                show_user: None
                                                            }
                                                        }
                                                    )
                                                };
                                                log_errors!(channel.send(message).await);
                                            }
                                        }
                                    }
                                }
                            },
                            Some(Err(err)) => {
                                error!("Frame decoder error: {}", err);
                                break;
                            },
                            None => {
                                debug!("The client has disconnected");
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

        (client, dispatcher)
    }

    /// Subscribe to DAP requests.
    pub fn subscribe_requests(&self) -> Result<broadcast::Receiver<(u32, RequestArguments)>, Error> {
        match self.requests_sender.upgrade() {
            Some(r) => Ok(r.subscribe()),
            None => Err("DAP session is gone".into()),
        }
    }

    /// Subscribe to DAP events.
    #[allow(unused)]
    pub fn subscribe_events(&self) -> Result<broadcast::Receiver<EventBody>, Error> {
        match self.events_sender.upgrade() {
            Some(r) => Ok(r.subscribe()),
            None => Err("DAP session is gone".into()),
        }
    }

    /// Send a reverse request to the DAP client.
    pub fn send_request(
        &self,
        request_args: RequestArguments,
    ) -> impl Future<Output = Result<ResponseBody, Error >> {
        let (resp_sender, resp_receiver) = oneshot::channel();
        let request = ProtocolMessageType::Request(request_args);
        let out_sender = self.out_sender.clone();
        async move {
            out_sender.send((request, Some(resp_sender))).await?;
            let result = resp_receiver.await?;
            match result {
                ResponseResult::Success { body } => Ok(body),
                ResponseResult::Error { message, .. } => Err(message.into()),
            }
        }
    }

    /// Send a response to the DAP client.
    #[allow(unused)]
    pub async fn send_response(&self, response: Response) -> Result<(), Error> {
        self.out_sender.send((ProtocolMessageType::Response(response), None)).await?;
        Ok(())
    }

    /// Send a response to the DAP client synchronously.
    pub fn try_send_response(&self, response: Response) -> Result<(), Error> {
        self.out_sender.try_send((ProtocolMessageType::Response(response), None))?;
        Ok(())
    }

    /// Sends an event to the DAP client.
    pub async fn send_event(&self, event_body: EventBody) -> Result<(), Error> {
        self.out_sender.send((ProtocolMessageType::Event(event_body), None)).await?;
        Ok(())
    }

    /// Send an event to the DAP client synchronously.
    pub fn try_send_event(&self, event_body: EventBody) -> Result<(), Error> {
        self.out_sender.try_send((ProtocolMessageType::Event(event_body), None))?;
        Ok(())
    }
}
