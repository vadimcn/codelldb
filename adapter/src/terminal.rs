use crate::prelude::*;

use crate::dap_session::DAPSession;
use crate::debug_protocol::*;
use std::time::Duration;
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

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
                args: args,
                cwd: String::new(),
                env: None,
                kind: Some(terminal_kind),
                title: Some(title),
            };
            let _resp = dap_session.send_request(RequestArguments::runInTerminal(req_args));

            let (stream, _remote_addr) = listener.accept().await?;

            let mut reader = BufReader::new(stream);
            let mut data = String::new();
            reader.read_line(&mut data).await?;

            Ok(Terminal {
                connection: reader.into_inner(),
                data: data.trim().to_owned(),
            })
        };

        match tokio::time::timeout(Duration::from_secs(5), terminal_fut).await {
            Ok(res) => res,
            Err(_) => bail!("Terminal agent did not respond within the allotted time."),
        }
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
