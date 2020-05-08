use bytes::BytesMut;
use log::{debug, error, info};
use std::fmt::Write;
use std::io;
use std::str;
use tokio_util::codec;

use crate::debug_protocol::ProtocolMessage;
use serde_json;

pub struct DAPCodec {
    state: State,
    content_len: usize,
}

enum State {
    ReadingHeaders,
    ReadingBody,
}

impl DAPCodec {
    pub fn new() -> DAPCodec {
        DAPCodec {
            state: State::ReadingHeaders,
            content_len: 0,
        }
    }
}

impl codec::Decoder for DAPCodec {
    type Item = ProtocolMessage;
    type Error = io::Error;

    fn decode(&mut self, buffer: &mut BytesMut) -> Result<Option<ProtocolMessage>, Self::Error> {
        loop {
            match self.state {
                State::ReadingHeaders => match buffer.windows(2).position(|b| b == &[b'\r', b'\n']) {
                    None => return Ok(None),
                    Some(pos) => {
                        let line = buffer.split_to(pos + 2);
                        if line.len() == 2 {
                            self.state = State::ReadingBody;
                        } else if let Ok(line) = str::from_utf8(&line) {
                            if line.len() > 15 && line[..15].eq_ignore_ascii_case("content-length:") {
                                if let Ok(content_len) = line[15..].trim().parse::<usize>() {
                                    self.content_len = content_len;
                                }
                            }
                        }
                    }
                },
                State::ReadingBody => {
                    if buffer.len() < self.content_len {
                        return Ok(None);
                    } else {
                        let message_bytes = buffer.split_to(self.content_len);
                        self.state = State::ReadingHeaders;
                        self.content_len = 0;

                        debug!("--> {}", str::from_utf8(&message_bytes).unwrap());
                        match serde_json::from_slice(&message_bytes) {
                            Ok(message) => return Ok(Some(message)),
                            Err(err) => {
                                error!("Could not deserialize: {}", err);
                                return Err(io::Error::new(io::ErrorKind::InvalidData, Box::new(err)));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl codec::Encoder<ProtocolMessage> for DAPCodec {
    type Error = io::Error;

    fn encode(&mut self, message: ProtocolMessage, buffer: &mut BytesMut) -> Result<(), Self::Error> {
        let message_bytes = serde_json::to_vec(&message).unwrap();
        debug!("<-- {}", str::from_utf8(&message_bytes).unwrap());

        buffer.reserve(32 + message_bytes.len());
        write!(buffer, "Content-Length: {}\r\n\r\n", message_bytes.len()).unwrap();
        buffer.extend_from_slice(&message_bytes);

        Ok(())
    }
}
