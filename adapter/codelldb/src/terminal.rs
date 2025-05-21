use crate::prelude::*;

use crate::dap_session::DAPSession;
use adapter_protocol::*;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};

pub struct Terminal {
    #[allow(unused)]
    connection: TcpStream,
    data: String,
}

impl Terminal {
    pub async fn create(
        terminal_kind: impl Into<String>,
        title: impl Into<String>,
        dap_session: DAPSession,
    ) -> Result<Terminal, Error> {
        let terminal_kind = terminal_kind.into();
        let title = title.into();

        let terminal_fut = async move {
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            let addr = listener.local_addr()?;

            let accept_fut = listener.accept();

            // Run codelldb in a terminal agent mode, which sends back the tty device name (Unix)
            // or its own process id (Windows), then waits till the socket gets closed from our end.
            let executable = std::env::current_exe()?.to_str().unwrap().into();
            let args = vec![
                executable,
                "terminal-agent".into(),
                format!("--connect={}", addr.port()),
            ];
            let req_args = RunInTerminalRequestArguments {
                args: args,
                cwd: String::new(),
                env: None,
                kind: Some(terminal_kind),
                title: Some(title),
                args_can_be_interpreted_by_shell: None,
            };
            let run_in_term = dap_session.send_request(RequestArguments::runInTerminal(req_args));
            tokio::task::spawn_local(run_in_term);

            let (stream, _remote_addr) = accept_fut.await?;
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

    #[cfg(windows)]
    pub fn attach_console(&self) {
        unsafe {
            let pid = self.data.parse::<u32>().unwrap();
            winapi::um::wincon::FreeConsole();
            winapi::um::wincon::AttachConsole(pid);
        }
    }

    #[cfg(windows)]
    pub fn detach_console(&self) {
        unsafe {
            winapi::um::wincon::FreeConsole();
        }
    }
}
