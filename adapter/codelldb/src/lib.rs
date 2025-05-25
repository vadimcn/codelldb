use crate::prelude::*;
use adapter_protocol::AdapterSettings;
use clap::ArgMatches;
use dap_session::DAPChannel;
use lldb::*;
use std::sync::Arc;
use std::{env, net};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Duration;
use tokio_util::codec::Decoder;

#[allow(unused_imports)]
mod prelude {
    pub use crate::error::{blame_nobody, blame_user, str_error, Blame, BlamedError, Error};
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
mod stdio_stream;
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

    SBDebugger::initialize();

    let debugger = SBDebugger::create(false);
    // Execute Python startup command
    if let Ok(command) = std::env::var("CODELLDB_STARTUP") {
        debug!("Executing {}", command);
        let mut command_result = SBCommandReturnObject::new();
        debugger.command_interpreter().handle_command(&command, &mut command_result, false);
    }

    let current_exe = env::current_exe().unwrap();
    let adapter_dir = current_exe.parent().unwrap();
    let python_interface = match python::initialize(&debugger, &adapter_dir) {
        Ok(python) => Some(python),
        Err(err) => {
            error!("Initialize Python interpreter: {}", err);
            None
        }
    };

    let (use_stdio, port, connect) = if let Some(port) = matches.value_of("connect") {
        (false, port.parse()?, true)
    } else if let Some(port) = matches.value_of("port") {
        (false, port.parse()?, false)
    } else {
        (true, 0, false)
    };
    let multi_session = matches.is_present("multi-session");
    let auth_token = matches.value_of("auth-token");

    let rt = tokio::runtime::Builder::new_multi_thread() //
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        if use_stdio {
            debug!("Starting on stdio");
            let stream = stdio_stream::StdioStream::new();
            let framed_stream = dap_codec::DAPCodec::new().framed(stream);
            run_debug_session(Box::new(framed_stream), &adapter_settings, &python_interface).await;
        } else {
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
                run_debug_session(Box::new(framed_stream), &adapter_settings, &python_interface).await;
            } else {
                let listener = TcpListener::bind(&addr).await?;
                while {
                    debug!("Listening on {}", listener.local_addr()?);
                    let (tcp_stream, _) = listener.accept().await?;
                    tcp_stream.set_nodelay(true).unwrap();
                    let framed_stream = dap_codec::DAPCodec::new().framed(tcp_stream);
                    run_debug_session(Box::new(framed_stream), &adapter_settings, &python_interface).await;
                    multi_session
                } {}
            }
        }
        Ok::<(), Error>(())
    })
    .unwrap();

    rt.shutdown_timeout(Duration::from_millis(10));

    debug!("Exiting");
    #[cfg(not(windows))]
    SBDebugger::terminate();
    Ok(())
}

async fn run_debug_session(
    framed_stream: Box<dyn DAPChannel>,
    adapter_settings: &adapter_protocol::AdapterSettings,
    python_interface: &Option<Arc<python::PythonInterface>>,
) {
    debug!("New debug session");
    let (dap_session, dap_fut) = dap_session::DAPSession::new(framed_stream);
    let session_fut = debug_session::DebugSession::run(
        dap_session,
        adapter_settings.clone(),
        python_interface.as_ref().map(|i| i.clone()),
    );
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

// Initialization for test binaries
#[cfg(test)]
#[ctor::ctor]
fn test_init() {
    use std::path::Path;
    lldb_stub::liblldb.load_from(Path::new(env!("LLDB_DYLIB"))).unwrap();
    lldb_stub::base.resolve().unwrap().mark_permanent();
    lldb_stub::v16.resolve().unwrap().mark_permanent();
}

#[cfg(test)]
lazy_static::lazy_static! {
    static ref TEST_DEBUGGER: SBDebugger = {
        use lldb::*;
        std::env::remove_var("PYTHONHOME");
        std::env::remove_var("PYTHONPATH");
        SBDebugger::initialize();
        SBDebugger::create(false)
    };
}
