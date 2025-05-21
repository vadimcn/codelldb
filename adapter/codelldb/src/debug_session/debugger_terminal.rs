use crate::prelude::*;

use crate::terminal::Terminal;

use lldb::*;
use std::fs::File;

#[cfg(unix)]
use std::os::unix::{io::AsRawFd, thread::JoinHandleExt};

#[allow(dead_code)]
pub(super) struct DebuggerTerminal {
    terminal: Terminal,
    thread: std::thread::JoinHandle<()>,
    input_fd: libc::c_int,
}

impl super::DebugSession {
    pub(super) fn create_debugger_terminal(&self, session_name: &str) {
        let title = format!("CodeLLDB: {}", session_name);
        let terminal_fut = Terminal::create(
            "integrated",
            title,
            self.dap_session.clone(),
        );
        let self_ref = self.self_ref.clone();
        let fut = async move {
            let result = terminal_fut.await;
            self_ref
                .map(|s| match result {
                    Ok(terminal) => {
                        #[cfg(windows)]
                        terminal.attach_console();

                        let stdin = File::open(terminal.input_devname()).unwrap();
                        let stdout = File::create(terminal.output_devname()).unwrap();
                        let stderr = File::create(terminal.output_devname()).unwrap();

                        #[cfg(unix)]
                        let stdin_fd = stdin.as_raw_fd();
                        #[cfg(windows)]
                        let stdin_fd = -1;

                        log_errors!(s.debugger.set_input_file(SBFile::from(stdin, false)));
                        log_errors!(s.debugger.set_output_file(SBFile::from(stdout, true)));
                        log_errors!(s.debugger.set_error_file(SBFile::from(stderr, true)));
                        let debugger = s.debugger.clone();
                        let thread = std::thread::Builder::new()
                            .name("command-interpreter".into())
                            .spawn(move || {
                                debugger.run_command_interpreter(false, false);
                                debug!("Exiting interpreter thread");
                            })
                            .unwrap();
                        s.debugger_terminal = Some(DebuggerTerminal {
                            terminal: terminal,
                            thread: thread,
                            input_fd: stdin_fd,
                        });
                    }
                    Err(err) => s.console_error(format!("Failed to launch a terminal for debugger console. ({})", err)),
                })
                .await;
        };
        tokio::task::spawn_local(fut);
    }

    pub(super) fn destroy_debugger_terminal(&mut self) -> Result<(), Error> {
        if let Some(dt) = self.debugger_terminal.take() {
            // We need to interrupt a blocking read() syscall wrapped in EINTR handling loop :(
            #[cfg(unix)]
            unsafe {
                let write_only = File::create("/dev/null")?;
                libc::pthread_kill(dt.thread.as_pthread_t(), libc::SIGUSR1);
                libc::dup2(write_only.as_raw_fd(), dt.input_fd);
                libc::pthread_kill(dt.thread.as_pthread_t(), libc::SIGUSR1);
            }
            // On Windows, simply detaching from console does the job.
            #[cfg(windows)]
            dt.terminal.detach_console();

            drop(dt.thread.join());
        }
        Ok(())
    }
}
