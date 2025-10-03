use std::env;
use std::io::{Read, Write};
use std::net;
use std::str::FromStr;

use clap::Parser;
use codelldb_types::TerminalId;
use codelldb_types::{JsonMap, LaunchEnvironment, LaunchResponse};

pub type Error = Box<dyn std::error::Error>;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    connect: Option<String>,
    #[arg(long)]
    config: Option<String>,
    #[arg(long)]
    clear_screen: bool,
    #[arg(trailing_var_arg = true)]
    cmd: Vec<String>,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let address = if let Some(address) = args.connect {
        address
    } else if let Ok(address) = env::var("CODELLDB_LAUNCH_CONNECT") {
        address
    } else {
        return Err("Need an address to connect to.".into());
    };

    let config = if let Some(config) = args.config {
        Some(config)
    } else if let Ok(config) = env::var("CODELLDB_LAUNCH_CONFIG") {
        Some(config)
    } else {
        None
    };

    let env = JsonMap(env::vars().collect::<Vec<_>>());

    #[cfg(unix)]
    let terminal_id = match get_tty_name() {
        Ok(name) => Some(TerminalId::TTY(name)),
        Err(_) => None,
    };

    #[cfg(windows)]
    let terminal_id = Some(TerminalId::PID(std::process::id() as u64));

    let request = LaunchEnvironment {
        cmd: args.cmd,
        cwd: std::env::current_dir().unwrap_or_default(),
        env: env,
        terminal_id: terminal_id,
        config: config,
    };

    let address = net::SocketAddr::from_str(&address)?;
    let mut stream = net::TcpStream::connect(address)?;
    serde_json::to_writer(&mut stream, &request)?;
    stream.flush()?;
    stream.shutdown(net::Shutdown::Write)?;

    if args.clear_screen {
        let _ = clearscreen::ClearScreen::default().clear();
    }

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    // Clear out any unread input buffered in stdin, so it doesn't get read by the shell.
    purge_stdin();

    match serde_json::from_str::<LaunchResponse>(&response) {
        Ok(response) => {
            if response.success {
                Ok(())
            } else {
                Err(response.message.unwrap_or("Failed".into()).into())
            }
        }
        Err(e) => Err(Box::new(e)),
    }
}

#[cfg(unix)]
fn purge_stdin() {
    use std::os::unix::io::AsRawFd;
    drop(termios::tcflush(std::io::stdin().as_raw_fd(), termios::TCIFLUSH));
}
#[cfg(windows)]
fn purge_stdin() {
    use std::os::windows::io::AsRawHandle;
    unsafe {
        winapi::um::wincon::FlushConsoleInputBuffer(std::io::stdin().as_raw_handle());
    }
}

#[cfg(unix)]
fn get_tty_name() -> Result<String, Error> {
    unsafe {
        let ptr = libc::ttyname(1);
        if ptr.is_null() {
            Err("No TTY".into())
        } else {
            let tty_name = std::ffi::CStr::from_ptr(ptr).to_str()?.to_owned();
            Ok(tty_name)
        }
    }
}

#[test]
fn test_args() {
    let args = Args::parse_from(["<launch>"]);
    assert_eq!(args.connect, None);
    assert_eq!(args.config, None);
    assert_eq!(args.clear_screen, false);
    assert_eq!(args.cmd, [""; 0]);

    let args = Args::parse_from(["<launch>", "command"]);
    assert_eq!(args.connect, None);
    assert_eq!(args.config, None);
    assert_eq!(args.clear_screen, false);
    assert_eq!(args.cmd, ["command"]);

    let args = Args::parse_from(["<launch>", "--clear-screen", "command"]);
    assert_eq!(args.connect, None);
    assert_eq!(args.config, None);
    assert_eq!(args.clear_screen, true);
    assert_eq!(args.cmd, ["command"]);

    let args = Args::parse_from(["<launch>", "command", "-arg", "val"]);
    assert_eq!(args.connect, None);
    assert_eq!(args.config, None);
    assert_eq!(args.clear_screen, false);
    assert_eq!(args.cmd, ["command", "-arg", "val"]);

    let args = Args::parse_from(["<launch>", "--", "-command"]);
    assert_eq!(args.connect, None);
    assert_eq!(args.config, None);
    assert_eq!(args.clear_screen, false);
    assert_eq!(args.cmd, ["-command"]);

    let args = Args::parse_from(["<launch>", "--connect=127.0.0.1:12345", "command", "-arg", "val"]);
    assert_eq!(args.connect.as_deref(), Some("127.0.0.1:12345"));
    assert_eq!(args.config, None);
    assert_eq!(args.clear_screen, false);
    assert_eq!(args.cmd, ["command", "-arg", "val"]);

    let args = Args::parse_from(["<launch>", "--connect=127.0.0.1:12345", "--", "--config", "-arg", "val"]);
    assert_eq!(args.connect.as_deref(), Some("127.0.0.1:12345"));
    assert_eq!(args.clear_screen, false);
    assert_eq!(args.cmd, ["--config", "-arg", "val"]);
}
