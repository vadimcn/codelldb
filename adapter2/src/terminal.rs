use crate::error::Error;
use log::debug;
use std::io::{self, BufRead};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

pub struct Terminal {
    connection: TcpStream,
    data: String,
}

impl Terminal {
    pub fn create<F>(run_in_terminal: F) -> Result<Self, Error>
    where
        F: FnOnce(Vec<String>) -> Result<(), Error>,
    {
        let mut listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;

        // Run codelldb in a terminal agent mode, which sends back the tty device name (Unix)
        // or its own process id (Windows), then waits till the socket gets closed from our end.
        let executable = std::env::current_exe()?.to_str().unwrap().into();
        let cmd = vec![executable, "terminal-agent".into(), format!("--port={}", addr.port())];
        run_in_terminal(cmd)?;

        let stream = accept_with_timeout(&mut listener, Duration::from_millis(5000))?;
        let stream2 = stream.try_clone()?;

        let mut reader = io::BufReader::new(stream);
        let mut data = String::new();
        reader.read_line(&mut data)?;

        Ok(Terminal {
            connection: stream2,
            data: data.trim().to_owned(),
        })
    }

    pub fn input_devname(&self) -> &str {
        if cfg!(windows) {
            "CONIN$"
        } else {
            &self.data
        }
    }

    pub fn output_devname(&self) -> &str {
        if cfg!(windows) {
            "CONOUT$"
        } else {
            &self.data
        }
    }

    pub fn attach<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        // Windows does not have an API for launching a child process attached to another console.
        // Instead,
        #[cfg(windows)]
        {
            use winapi::um::wincon::{AttachConsole, FreeConsole};
            let pid = self.data.parse::<u32>().unwrap();
            unsafe {
                dbg!(FreeConsole());
                dbg!(AttachConsole(pid));
            }
            let result = f();
            unsafe {
                dbg!(FreeConsole());
            }
            result
        }

        #[cfg(not(windows))]
        f()
    }
}

// No set_accept_timeout() in std :(
fn accept_with_timeout(listener: &mut TcpListener, timeout: Duration) -> Result<TcpStream, Error> {
    listener.set_nonblocking(true)?;
    let timeout = Duration::from_millis(5000);
    let started = Instant::now();
    let stream = loop {
        match listener.accept() {
            Ok((stream, _addr)) => break stream,
            Err(e) => {
                if e.kind() != io::ErrorKind::WouldBlock {
                    bail!(e);
                } else {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
        if started.elapsed() > timeout {
            bail!("Terminal agent did not respond within the allotted time.");
        }
    };
    stream.set_nonblocking(false)?;
    Ok(stream)
}
