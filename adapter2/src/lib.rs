#![feature(try_trait)]
#![feature(fnbox)]
#![allow(unused)]

use std::net;

use futures::prelude::*;
use tokio::prelude::*;

use log::{debug, error, info};
use tokio::codec::Decoder;
use tokio::io;
use tokio::net::TcpListener;

use lldb::*;

mod cancellation;
mod debug_protocol;
mod debug_session;
mod disassembly;
mod error;
mod expressions;
mod handles;
mod must_initialize;
mod python;
mod fsutil;
mod stdio_channel;
mod terminal;
mod wire_protocol;

#[no_mangle]
pub extern "C" fn entry(port: u16, multi_session: bool, adapter_params: Option<&str>) {
    env_logger::Builder::from_default_env().init();
    SBDebugger::initialize();

    let adapter_params: debug_session::AdapterParameters = match adapter_params {
        Some(s) => serde_json::from_str(s).unwrap(),
        None => Default::default(),
    };

    let localhost = net::Ipv4Addr::new(127, 0, 0, 1);
    let addr = net::SocketAddr::new(localhost.into(), port);
    let listener = TcpListener::bind(&addr).unwrap();

    println!("Listening on port {}", listener.local_addr().unwrap().port());

    let server = listener.incoming().map_err(|err| {
        error!("accept error: {:?}", err);
        panic!()
    });

    let server: Box<Stream<Item = _, Error = _> + Send> = if !multi_session {
        Box::new(server.take(1))
    } else {
        Box::new(server)
    };

    let server = server
        .for_each(move |conn| {
            conn.set_nodelay(true).unwrap();
            run_debug_session(conn, adapter_params.clone())
        })
        .then(|r| {
            info!("### server resolved: {:?}", r);
            Ok(())
        });

    tokio::run(server);
    debug!("### Exiting");
    #[cfg(not(windows))]
    SBDebugger::terminate();
}

fn run_debug_session(
    stream: impl AsyncRead + AsyncWrite + Send + 'static, adapter_params: debug_session::AdapterParameters,
) -> impl Future<Item = (), Error = io::Error> {
    future::lazy(|| {
        debug!("New debug session");

        let (to_client, from_client) = wire_protocol::Codec::new().framed(stream).split();
        let (to_session, from_session) = debug_session::DebugSession::new(adapter_params).split();

        let client_to_session = from_client
            .map_err(|_| ()) //.
            .forward(to_session)
            .then(|_| {
                info!("### client_to_session stream terminated");
                Ok(())
            });
        tokio::spawn(client_to_session);

        let session_to_client = from_session
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "DebugSession error"))
            .forward(to_client)
            .then(|_| {
                debug!("### session_to_client stream terminated");
                Ok(())
            });

        session_to_client
    })
}
