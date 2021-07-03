use crate::prelude::*;
use futures::prelude::*;
use lldb::*;
use std::net;
use tokio::net::TcpListener;
use tokio::time::Duration;
use tokio_util::codec::Decoder;

#[allow(unused_imports)]
mod prelude {
    pub use crate::error::{as_user_error, Error};
    pub use log::{debug, error, info};
}

#[macro_use]
mod error;
mod cancellation;
mod dap_codec;
mod dap_session;
mod debug_event_listener;
mod debug_protocol;
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
mod vec_map;

#[no_mangle]
#[allow(improper_ctypes_definitions)]
pub extern "C" fn entry(port: u16, multi_session: bool, adapter_params: Option<&str>) {
    hook_crashes();
    env_logger::Builder::from_default_env().format_timestamp_millis().init();

    SBDebugger::initialize();

    let adapter_settings: debug_protocol::AdapterSettings = match adapter_params {
        Some(s) => serde_json::from_str(s).unwrap(),
        None => Default::default(),
    };

    // Execute startup command
    if let Ok(command) = std::env::var("CODELLDB_STARTUP") {
        let debugger = SBDebugger::create(false);
        let mut command_result = SBCommandReturnObject::new();
        debugger.command_interpreter().handle_command(&command, &mut command_result, false);
    }

    let localhost = net::Ipv4Addr::new(127, 0, 0, 1);
    let addr = net::SocketAddr::new(localhost.into(), port);

    let rt = tokio::runtime::Builder::new_multi_thread() //
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(run_debug_server(addr, adapter_settings, multi_session));
    rt.shutdown_timeout(Duration::from_millis(10));

    debug!("Exiting");
    #[cfg(not(windows))]
    SBDebugger::terminate();
}

async fn run_debug_server(
    addr: net::SocketAddr,
    adapter_settings: debug_protocol::AdapterSettings,
    multi_session: bool,
) {
    let listener = TcpListener::bind(&addr).await.unwrap();

    println!("Listening on port {}", listener.local_addr().unwrap().port());

    loop {
        let (tcp_stream, _) = listener.accept().await.unwrap();
        debug!("New debug session");
        tcp_stream.set_nodelay(true).unwrap();
        let framed_stream = dap_codec::DAPCodec::new().framed(tcp_stream);
        let (dap_session, dap_fut) = dap_session::DAPSession::new(Box::new(framed_stream));
        let session_fut = debug_session::DebugSession::run(dap_session, adapter_settings.clone());
        tokio::spawn(dap_fut);
        session_fut.await;
        debug!("Session has ended");
        if !multi_session {
            break;
        }
    }
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
