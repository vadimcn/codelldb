use crate::prelude::*;

use crate::dap_session::DAPSession;
use adapter_protocol::*;
use std::collections::HashMap;
use std::net::TcpStream;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;

pub struct Terminal {
    #[allow(unused)]
    connection: TcpStream,
    terminal_id: Either<Option<String>, u64>,
}

impl Terminal {
    pub async fn create(
        terminal_kind: RunInTerminalRequestArgumentsKind,
        title: impl Into<String>,
        dap_session: DAPSession,
    ) -> Result<Terminal, Error> {
        let terminal_kind = terminal_kind.into();
        let title = title.into();

        let terminal_fut = async move {
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            let addr = listener.local_addr()?;

            let accept_fut = listener.accept();

            let current_exe = std::env::current_exe()?;
            let mut launcher = current_exe.with_file_name("codelldb-launch");
            if let Some(ext) = current_exe.extension() {
                launcher.set_extension(ext);
            }
            let req_args = RunInTerminalRequestArguments {
                args: vec![
                    launcher.to_string_lossy().to_string(),
                    format!("--connect={addr}"),
                    "--clear-screen".into(),
                ],
                cwd: String::new(),
                env: HashMap::new(),
                kind: Some(terminal_kind),
                title: Some(title),
                args_can_be_interpreted_by_shell: None,
            };
            let run_in_term = dap_session.send_request(RequestArguments::runInTerminal(req_args));
            tokio::task::spawn_local(run_in_term);

            let (stream, _remote_addr) = accept_fut.await?;
            let mut reader = BufReader::new(stream);
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).await?;

            let launch_env: LaunchEnvironment = serde_json::from_slice(&buf)?;

            Ok(Terminal {
                connection: reader.into_inner().into_std()?,
                terminal_id: launch_env.terminal_id,
            })
        };

        match tokio::time::timeout(Duration::from_secs(5), terminal_fut).await {
            Ok(res) => res,
            Err(_) => bail!("Terminal agent did not respond within the allotted time."),
        }
    }

    pub fn input_devname(&self) -> Option<&str> {
        if cfg!(windows) {
            Some("CONIN$")
        } else if let Either::First(ref tty_name) = self.terminal_id {
            tty_name.as_deref()
        } else {
            None
        }
    }

    pub fn output_devname(&self) -> Option<&str> {
        if cfg!(windows) {
            Some("CONOUT$")
        } else if let Either::First(ref tty_name) = self.terminal_id {
            tty_name.as_deref()
        } else {
            None
        }
    }

    #[cfg(windows)]
    pub fn attach_console(&self) -> bool {
        use winapi::shared::minwindef::DWORD;
        if let Either::Second(pid) = self.terminal_id {
            unsafe {
                winapi::um::wincon::FreeConsole();
                winapi::um::wincon::AttachConsole(pid as DWORD) != 0
            }
        } else {
            false
        }
    }

    #[cfg(windows)]
    pub fn detach_console(&self) {
        unsafe {
            winapi::um::wincon::FreeConsole();
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let response = LaunchResponse {
            success: true,
            message: None,
        };
        log_errors!(serde_json::to_writer(&mut self.connection, &response));
    }
}
