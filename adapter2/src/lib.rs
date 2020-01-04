#![feature(try_trait)]
#![feature(fn_traits)]
#![feature(untagged_unions)]
#![allow(unused)]

use futures::prelude::*;
use tokio::prelude::*;

use log::{debug, error, info};
use std::net;
use tokio::codec::Decoder;
use tokio::io;
use tokio::net::TcpListener;

use crate::error::Error;
use lldb::*;

#[macro_use]
mod error;
mod cancellation;
mod debug_protocol;
mod debug_session;
mod disassembly;
mod expressions;
mod fsutil;
mod handles;
mod must_initialize;
mod python;
mod stdio_channel;
mod terminal;
mod vec_map;
mod wire_protocol;

#[no_mangle]
pub extern "C" fn entry(port: u16, multi_session: bool, adapter_params: Option<&str>) {
    hook_crashes();
    env_logger::Builder::from_default_env().init();

    SBDebugger::initialize();

    let adapter_settings: debug_protocol::AdapterSettings = match adapter_params {
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

    let server: Box<dyn Stream<Item = _, Error = _> + Send> = if !multi_session {
        Box::new(server.take(1))
    } else {
        Box::new(server)
    };

    let server = server
        .for_each(move |conn| {
            conn.set_nodelay(true).unwrap();
            run_debug_session(conn, adapter_settings.clone())
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
    stream: impl AsyncRead + AsyncWrite + Send + 'static,
    adapter_settings: debug_protocol::AdapterSettings
) -> impl Future<Item = (), Error = io::Error> {
    future::lazy(move || {
        debug!("New debug session");

        let (to_client, from_client) = wire_protocol::Codec::new().framed(stream).split();
        let (to_session, from_session) = debug_session::DebugSession::new(adapter_settings).split();

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

#[cfg(unix)]
fn hook_crashes() {
    extern "C" fn handler(sig: libc::c_int) {
        let sig_name = match sig {
            libc::SIGSEGV => "SIGSEGV",
            libc::SIGBUS => "SIGBUS",
            libc::SIGILL => "SIGILL",
            libc::SIGFPE => "SIGFPE",
            libc::SIGABRT => "SIGABRT",
            _ => unreachable!(),
        };
        let bt = backtrace::Backtrace::new();
        eprintln!("Received signal: {}", sig_name);
        eprintln!("{:?}", bt);
        std::process::exit(255);
    }

    unsafe {
        libc::signal(libc::SIGSEGV, handler as usize);
        libc::signal(libc::SIGBUS, handler as usize);
        libc::signal(libc::SIGILL, handler as usize);
        libc::signal(libc::SIGFPE, handler as usize);
        libc::signal(libc::SIGABRT, handler as usize);
    }
}

#[cfg(windows)]
fn hook_crashes() {}
