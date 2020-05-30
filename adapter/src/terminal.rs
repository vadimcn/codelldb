use log::debug;
use std::io::{self, BufRead};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use crate::dap_session::DAPSession;
use crate::debug_protocol::*;
use crate::error::Error;

pub struct Terminal {
    #[allow(unused)]
    connection: TcpStream,
    data: String,
}

impl Terminal {
    pub async fn create(
        terminal_kind: impl Into<String>,
        title: impl Into<String>,
        clear_sequence: Option<Vec<String>>,
        mut dap_session: DAPSession,
    ) -> Result<Terminal, Error> {
        let terminal_kind = terminal_kind.into();
        let title = title.into();

        let terminal_fut = async {
            if let Some(clear_sequence) = clear_sequence {
                let req_args = RunInTerminalRequestArguments {
                    args: clear_sequence,
                    cwd: String::new(),
                    env: None,
                    kind: Some(terminal_kind.clone()),
                    title: Some(title.clone()),
                };
                dap_session.send_request(RequestArguments::runInTerminal(req_args)).await?;
            }

            let mut listener = TcpListener::bind("127.0.0.1:0").await?;
            let addr = listener.local_addr()?;

            // Run codelldb in a terminal agent mode, which sends back the tty device name (Unix)
            // or its own process id (Windows), then waits till the socket gets closed from our end.
            let executable = std::env::current_exe()?.to_str().unwrap().into();
            let args = vec![executable, "terminal-agent".into(), format!("--port={}", addr.port())];
            let req_args = RunInTerminalRequestArguments {
                args: vec![reset_sequence.into()],
                cwd: String::new(),
                env: None,
                kind: Some(terminal_kind.clone()),
                title: Some(title.clone()),
            };
            dap_session.send_request(RequestArguments::runInTerminal(req_args)).await?;
        }

        let mut listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;

        // Run codelldb in a terminal agent mode, which sends back the tty device name (Unix)
        // or its own process id (Windows), then waits till the socket gets closed from our end.
        let executable = std::env::current_exe()?.to_str().unwrap().into();
        let args = vec![executable, "terminal-agent".into(), format!("--port={}", addr.port())];
        let req_args = RunInTerminalRequestArguments {
            args: args,
            cwd: String::new(),
            env: None,
            kind: Some(terminal_kind),
            title: Some(title),
        };
        dap_session.send_request(RequestArguments::runInTerminal(req_args)).await?;

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
