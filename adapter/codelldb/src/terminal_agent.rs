use clap::ArgMatches;
use std::io::{Read, Write};
use std::net;

use super::Error;

pub fn terminal_agent(matches: &ArgMatches) -> Result<(), Error> {
    let data;
    #[cfg(unix)]
    {
        unsafe {
            let ptr = libc::ttyname(1);
            assert!(!ptr.is_null());
            data = std::ffi::CStr::from_ptr(ptr).to_str()?;
        }
    }
    #[cfg(windows)]
    {
        data = std::process::id();
    }

    let port: u16 = matches.value_of("connect").unwrap().parse().unwrap();
    let addr = net::SocketAddr::new(net::Ipv4Addr::new(127, 0, 0, 1).into(), port);
    let mut stream = net::TcpStream::connect(addr)?;
    writeln!(stream, "{}", data)?;

    clear_screen();

    // Wait for the other end to close connection (which will be maintained till the end of
    // the debug session; this prevents terminal shell from stealing debuggee's input form stdin).
    for b in stream.bytes() {
        if let Err(_) = b {
            break;
        }
    }

    // Clear out any unread input buffered in stdin, so it doesn't get read by the shell.
    purge_stdin();

    Ok(())
}

fn clear_screen() {
    let terminal = crossterm::terminal();
    drop(terminal.clear(crossterm::ClearType::All));
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
