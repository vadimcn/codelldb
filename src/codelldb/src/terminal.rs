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
    connection: Option<TcpStream>,
    terminal_id: Option<TerminalId>,
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
            if !launcher.exists() {
                launcher.pop(); // filename
                launcher.pop(); // directory
                launcher.push("bin");
                launcher.push("codelldb-launch");
                if let Some(ext) = current_exe.extension() {
                    launcher.set_extension(ext);
                }
            }
            if !launcher.exists() {
                bail!("Could not find codelldb-launch");
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
                connection: Some(reader.into_inner().into_std()?),
                terminal_id: launch_env.terminal_id,
            })
        };

        match tokio::time::timeout(Duration::from_secs(10), terminal_fut).await {
            Ok(res) => res,
            Err(_) => bail!("Terminal agent did not respond within the allotted time."),
        }
    }

    pub fn from_terminal_id(terminal_id: TerminalId) -> Terminal {
        Terminal {
            connection: None,
            terminal_id: Some(terminal_id),
        }
    }

    pub fn input_devname(&self) -> Option<&str> {
        if cfg!(windows) {
            Some("CONIN$")
        } else if let Some(TerminalId::TTY(ref tty_name)) = self.terminal_id {
            Some(tty_name)
        } else {
            None
        }
    }

    pub fn output_devname(&self) -> Option<&str> {
        if cfg!(windows) {
            Some("CONOUT$")
        } else if let Some(TerminalId::TTY(ref tty_name)) = self.terminal_id {
            Some(tty_name)
        } else {
            None
        }
    }

    #[cfg(windows)]
    pub fn attach_console(&self) -> Result<(), Error> {
        use winapi::shared::minwindef::DWORD;
        if let Some(TerminalId::PID(pid)) = self.terminal_id {
            unsafe {
                winapi::um::wincon::FreeConsole();
                if winapi::um::wincon::AttachConsole(pid as DWORD) != 0 {
                    Ok(())
                } else {
                    let err = winapi::um::errhandlingapi::GetLastError();
                    bail!(format!("Could not attach to console (err=0x{err:08X})"))
                }
            }
        } else {
            bail!("No console process id.")
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
        if let Some(ref connection) = self.connection {
            let response = LaunchResponse {
                success: true,
                message: None,
            };
            log_errors!(serde_json::to_writer(connection, &response));
        }
    }
}
