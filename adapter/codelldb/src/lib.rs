use crate::prelude::*;
use adapter_protocol::{AdapterSettings, Either};
use clap::ArgMatches;
use dap_session::DAPChannel;
use lldb::*;
use std::net;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Duration;
use tokio_util::codec::Decoder;

#[allow(unused_imports)]
mod prelude {
    pub use crate::error::{as_user_error, Error};
    pub use log::{debug, error, info, warn};
}
#[macro_use]
mod error;
mod cancellation;
mod dap_codec;
mod dap_session;
mod debug_event_listener;
mod debug_session;
mod disassembly;
mod expressions;
mod fsutil;
mod handles;
mod must_initialize;
mod platform;
mod python;
mod shared;
mod terminal;

pub fn debug_server(matches: &ArgMatches) -> Result<(), Error> {
    hook_crashes();

    let adapter_settings: AdapterSettings = match matches.value_of("settings") {
        Some(s) => match serde_json::from_str(s) {
            Ok(settings) => settings,
            Err(err) => {
                error!("{}", err);
                Default::default()
            }
        },
        None => Default::default(),
    };

    match adapter_settings.reproducer {
        Some(Either::First(true)) => initialize_reproducer(None),
        Some(Either::Second(ref path)) => initialize_reproducer(Some(Path::new(&path))),
        _ => {}
    }

    SBDebugger::initialize();

    // Execute startup command
    if let Ok(command) = std::env::var("CODELLDB_STARTUP") {
        debug!("Executing {}", command);
        let debugger = SBDebugger::create(false);
        let mut command_result = SBCommandReturnObject::new();
        debugger.command_interpreter().handle_command(&command, &mut command_result, false);
    }

    let run_mode = if let Some(port) = matches.value_of("connect") {
        RunMode::Tcp {
            port: port.parse()?,
            connect: true,
        }
    } else if let Some(port) = matches.value_of("port") {
        RunMode::Tcp {
            port: port.parse()?,
            connect: false,
        }
    } else {
        RunMode::StdInOut
    };

    let rt = tokio::runtime::Builder::new_multi_thread() //
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        match run_mode {
            RunMode::Tcp { port, connect } => {
                let multi_session = matches.is_present("multi-session");
                let auth_token = matches.value_of("auth-token");
                let localhost = net::Ipv4Addr::new(127, 0, 0, 1);
                let addr = net::SocketAddr::new(localhost.into(), port);
                if connect {
                    debug!("Connecting to {}", addr);
                    let mut tcp_stream = TcpStream::connect(addr).await?;
                    tcp_stream.set_nodelay(true).unwrap();
                    if let Some(auth_token) = auth_token {
                        let auth_header = format!("Auth-Token: {}\r\n", auth_token);
                        tcp_stream.write_all(&auth_header.as_bytes()).await?;
                    }
                    let framed_stream = dap_codec::DAPCodec::new().framed(tcp_stream);
                    run_debug_session(Box::new(framed_stream), adapter_settings.clone()).await;
                } else {
                    let listener = TcpListener::bind(&addr).await?;
                    while {
                        debug!("Listening on {}", listener.local_addr()?);
                        let (tcp_stream, _) = listener.accept().await?;
                        tcp_stream.set_nodelay(true).unwrap();
                        let framed_stream = dap_codec::DAPCodec::new().framed(tcp_stream);
                        run_debug_session(Box::new(framed_stream), adapter_settings.clone()).await;
                        multi_session
                    } {}
                }
                Ok::<(), Error>(())
            }

            RunMode::StdInOut => {
                tokio::io::stdin();
                let std_in = tokio::io::stdin();
                let std_out = tokio::io::stdout();

                let std_in_out = crate::StdInOut { std_in, std_out };
                let framed_stream = dap_codec::DAPCodec::new().framed(std_in_out);
                run_debug_session(Box::new(framed_stream), adapter_settings.clone()).await;
                Ok::<(), Error>(())
            }
        }
    })
    .unwrap();

    rt.shutdown_timeout(Duration::from_millis(10));

    finalize_reproducer();
    debug!("Exiting");
    #[cfg(not(windows))]
    SBDebugger::terminate();
    Ok(())
}

async fn run_debug_session(framed_stream: Box<dyn DAPChannel>, adapter_settings: adapter_protocol::AdapterSettings) {
    debug!("New debug session");
    let (dap_session, dap_fut) = dap_session::DAPSession::new(framed_stream);
    let session_fut = debug_session::DebugSession::run(dap_session, adapter_settings.clone());
    tokio::spawn(dap_fut);
    session_fut.await;
    debug!("End of the debug session");
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
        finalize_reproducer();
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

static CREATING_REPRODUCER: AtomicBool = AtomicBool::new(false);

fn initialize_reproducer(path: Option<&Path>) {
    match SBReproducer::capture(path) {
        Ok(()) => CREATING_REPRODUCER.store(true, Ordering::Release),
        Err(err) => error!("initialize_reproducer: {}", err),
    }
}

fn finalize_reproducer() {
    if CREATING_REPRODUCER.load(Ordering::Acquire) {
        if let Some(path) = SBReproducer::path() {
            if SBReproducer::generate() {
                info!("Reproducer saved to {:?}", path);
            } else {
                error!("finalize_reproducer: failed");
            }
        }
    }
}

enum RunMode {
    Tcp { port: u16, connect: bool },
    StdInOut,
}

pub struct StdInOut {
    std_in: tokio::io::Stdin,
    std_out: tokio::io::Stdout,
}

impl AsyncRead for StdInOut {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().std_in).poll_read(cx, buf)
    }
}

impl AsyncWrite for StdInOut {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.get_mut().std_out).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().std_out).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().std_out).poll_shutdown(cx)
    }
}
