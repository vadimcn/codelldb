use crate::prelude::*;

use crate::fsutil::lldb_quoted_string;
use crate::must_initialize::Initialized;

use super::*;
use adapter_protocol::*;
use lldb::*;

impl super::DebugSession {
    pub(super) fn report_launch_cfg_error(&mut self, err: serde_json::Error) -> Result<ResponseBody, Error> {
        bail!(blame_user(str_error(format!(
            "Could not parse launch configuration: {}",
            err
        ))))
    }

    pub(super) fn handle_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        self.common_init_session(&args.common)?;

        self.no_debug = args.no_debug.unwrap_or(false);
        if self.no_debug {
            // Disable symbol pre-loading to speed up no-debug startup.
            log_errors!(self.debugger.set_variable("target.preload-symbols", "false"));
            // Attempts to set a breakpoint on __jit_debug_register_code in each module, thus causing symbol loading.
            log_errors!(self.debugger.set_variable("plugin.jit-loader.gdb.enable", "off"));
        }

        let target = if let Some(commands) = &args.target_create_commands {
            self.exec_commands("targetCreateCommands", &commands)?;
            let target = self.debugger.selected_target();
            if target.is_valid() {
                target
            } else {
                self.debugger.create_target(None, None, None, false)?
            }
        } else {
            let program = match &args.program {
                Some(program) => program,
                None => bail!(blame_user(str_error(
                    "The \"program\" attribute is required for launch."
                ))),
            };
            self.create_target_from_program(program)?
        };
        self.set_target(target);

        self.send_event(EventBody::initialized);

        let mut config_done_recv = self.configuration_done_sender.subscribe();
        let self_ref = self.self_ref.clone();
        let fut = async move {
            // Work around https://github.com/microsoft/vscode/issues/231074 by pausing before sending the
            // `runInTerminal` message.  This gives VSCode the chance to process `output` messages we sent earlier.
            tokio::time::sleep(time::Duration::from_millis(100)).await;
            log_errors!(config_done_recv.recv().await);
            self_ref.map(|s| s.create_terminal(&args)).await.await;
            self_ref.map(|s| s.complete_launch(args)).await
        };
        Err(AsyncResponse(Box::new(fut)).into())
    }

    fn complete_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        let mut launch_info = self.target.launch_info();
        let platform = self.debugger.selected_platform();

        let mut fold_key = make_env_key_folder(platform.triple().contains("windows"));

        let inherit_env = match self.debugger.get_variable("target.inherit-env").string_at_index(0) {
            Some("true") => true,
            _ => false,
        };

        // Init with platform environment if `inherit-env` is set.
        let launch_env = if inherit_env { platform.environment() } else { SBEnvironment::new() };
        if let Some(ref env_file) = args.env_file {
            match std::fs::File::open(env_file) {
                Ok(file) => {
                    for item in dotenvy::from_read_iter(file) {
                        let (k, v) = item?;
                        launch_env.set(fold_key(&k), v, true);
                    }
                }
                Err(err) => self.console_error(format!("Could not read {env_file}: {err}")),
            }
        }
        if let Some(ref env) = args.env {
            for (k, v) in env.iter() {
                launch_env.set(fold_key(k), v, true);
            }
        }
        debug!("launch env: {:?}", launch_env);
        launch_info.set_environment(&launch_env, false);

        if let Some(ref args) = args.args {
            launch_info.set_arguments(args.iter().map(|a| a.as_ref()), false);
        }
        if let Some(ref cwd) = args.cwd {
            launch_info.set_working_directory(Path::new(&cwd));
        }
        if let Some(true) = args.common.stop_on_entry {
            launch_info.set_launch_flags(launch_info.launch_flags() | LaunchFlag::StopAtEntry);
        }
        self.configure_stdio(&args, &mut launch_info)?;
        self.target.set_launch_info(&launch_info);

        // Run user commands (which may modify launch info)
        if let Some(ref commands) = args.common.pre_run_commands {
            self.exec_commands("preRunCommands", commands)?;
        }
        // Grab updated launch info.
        let launch_info = self.target.launch_info();

        // Announce the final launch command line
        let executable = self.target.executable().path().to_string_lossy().into_owned();
        let command_line = launch_info.arguments().fold(executable, |mut args, a| {
            args.push(' ');
            args.push_str(a);
            args
        });
        self.console_message(format!("Launching: {}", command_line));

        #[cfg(target_os = "linux")]
        {
            // The personality() syscall is often restricted inside Docker containers, which causes launch failure with a cryptic error.
            // Test if ASLR can be disabled and turn DisableASLR off if so.
            let flags = launch_info.launch_flags();
            if flags.contains(LaunchFlag::DisableASLR) {
                unsafe {
                    const ADDR_NO_RANDOMIZE: libc::c_ulong = 0x0040000;
                    let previous = libc::personality(0xffffffff) as libc::c_ulong;
                    if libc::personality(previous | ADDR_NO_RANDOMIZE) < 0 {
                        launch_info.set_launch_flags(flags - LaunchFlag::DisableASLR);
                        self.console_error("Could not disable address space layout randomization (ASLR).");
                        self.console_message("(Possibly due to running in a restricted container. \
                            Add \"initCommands\":[\"settings set target.disable-aslr false\"] to the launch configuration \
                            to suppress this warning.)",
                        );
                    }
                    libc::personality(previous);
                }
            }
        }

        // On Windows we can't specify the console to attach to when launching a process.
        // Instead, child inherits the console of the parent process, so we need to attach/detach around the launch.
        #[cfg(windows)]
        if let Some(terminal) = &self.debuggee_terminal {
            terminal.attach_console();
        }

        let launch_result: Result<SBProcess, Error> = (|| match &args.process_create_commands {
            None => self.target.launch(&launch_info).map_err(|sberr| Box::new(sberr).into()),
            Some(commands) => {
                self.exec_commands("processCreateCommands", commands)?;
                if self.debugger.selected_target() != self.target {
                    self.set_target(self.debugger.selected_target());
                }
                let process = self.target.process();
                self.check_process_create_commands(&process)?;
                Ok(process)
            }
        })();

        #[cfg(windows)]
        if let Some(terminal) = &self.debuggee_terminal {
            terminal.detach_console();
        }

        if let Err(err) = launch_result {
            let mut err = blame_user(err);
            if let Some(work_dir) = launch_info.working_directory() {
                if self.target.platform().get_file_permissions(work_dir) == 0 {
                    err.inner = str_error(format!(
                        "{}\n\nPossible cause: the working directory \"{}\" is missing or inaccessible.",
                        err.inner,
                        work_dir.display()
                    ));
                }
            }
            bail!(err);
        };

        self.console_message(format!(
            "Launched process {} from '{:?}'",
            self.target.process().process_id(),
            self.target.executable(),
        ));

        self.terminate_on_disconnect = true;
        self.common_post_run(args.common)?;

        Ok(ResponseBody::launch)
    }

    pub(super) fn handle_attach(&mut self, args: AttachRequestArguments) -> Result<ResponseBody, Error> {
        self.common_init_session(&args.common)?;

        if args.program.is_none() && args.pid.is_none() && args.target_create_commands.is_none() {
            bail!(blame_user(str_error(
                "Either \"program\" or \"pid\" is required to attach."
            )));
        }

        let target = match &args.target_create_commands {
            None => {
                if let Some(program) = &args.program {
                    // Try to create target from `program`.
                    if let Ok(target) = self.create_target_from_program(program) {
                        target
                    } else {
                        // Assume they are doing attach-by-name
                        self.debugger.create_target(None, None, None, false)?
                    }
                } else {
                    // Ok if attach-by-pid
                    self.debugger.create_target(None, None, None, false)?
                }
            }
            Some(commands) => {
                self.exec_commands("targetCreateCommands", &commands)?;
                let target = self.debugger.selected_target();
                if target.is_valid() {
                    target
                } else {
                    self.debugger.create_target(None, None, None, false)?
                }
            }
        };
        self.set_target(target);

        self.send_event(EventBody::initialized);

        let mut config_done_recv = self.configuration_done_sender.subscribe();
        let self_ref = self.self_ref.clone();
        let fut = async move {
            log_errors!(config_done_recv.recv().await);
            self_ref.map(|s| s.complete_attach(args)).await
        };
        Err(AsyncResponse(Box::new(fut)).into())
    }

    fn complete_attach(&mut self, args: AttachRequestArguments) -> Result<ResponseBody, Error> {
        if let Some(commands) = &args.common.pre_run_commands {
            self.exec_commands("preRunCommands", commands)?;
        }

        let process = match &args.process_create_commands {
            None => {
                let attach_info = SBAttachInfo::new();
                if let Some(pid) = &args.pid {
                    let pid = match pid {
                        Pid::Number(n) => *n as ProcessID,
                        Pid::String(s) => match s.parse() {
                            Ok(n) => n,
                            Err(_) => bail!(blame_user(str_error("Process id must be a positive integer."))),
                        },
                    };
                    attach_info.set_process_id(pid);
                } else {
                    let executable = self.target.executable();
                    if executable.is_valid() {
                        attach_info.set_executable(&executable.path());
                    } else if let Some(program) = &args.program {
                        attach_info.set_executable(Path::new(program));
                    } else {
                        bail!("unreachable");
                    }
                }

                attach_info.set_wait_for_launch(args.wait_for.unwrap_or(false), false);
                attach_info.set_ignore_existing(false);

                let process = match self.target.attach(&attach_info) {
                    Ok(process) => process,
                    Err(err) => bail!(blame_user(str_error(format!("Could not attach: {}", err)))),
                };

                if args.common.stop_on_entry.unwrap_or(false) {
                    self.notify_process_stopped(); // LLDB won't generate event for the initial stop
                } else {
                    log_errors!(process.resume());
                }
                process
            }
            Some(commands) => {
                self.exec_commands("processCreateCommands", commands)?;
                if self.debugger.selected_target() != self.target {
                    self.set_target(self.debugger.selected_target());
                }
                let process = self.target.process();
                self.check_process_create_commands(&process)?;
                self.target.process()
            }
        };

        self.console_message(format!("Attached to process {}", process.process_id()));

        self.terminate_on_disconnect = false;
        self.common_post_run(args.common)?;

        Ok(ResponseBody::attach)
    }

    // Check state of the process created via processCreateCommands
    fn check_process_create_commands(&mut self, process: &SBProcess) -> Result<(), Error> {
        let state = process.state();
        if !state.is_alive() {
            self.notify_process_terminated();
            bail!(blame_user(str_error("Process is not valid.")));
        } else if !state.is_running() {
            let event = process.stop_event_for_stop_id(process.stop_id(false));
            if let Some(process_event) = event.as_process_event() {
                if !process_event.restarted() {
                    self.notify_process_stopped();
                }
            }
        }
        Ok(())
    }

    pub(super) fn handle_restart(&mut self, args: RestartRequestArguments) -> Result<(), Error> {
        if let Some(commands) = &self.pre_terminate_commands {
            self.exec_commands("preTerminateCommands", &commands)?;
        }

        self.debug_event_listener.cork();
        self.terminate_debuggee(None)?;
        self.debug_event_listener.uncork();

        match args.arguments {
            Either::First(args) => {
                self.complete_launch(args)?;
            }
            Either::Second(args) => {
                self.complete_attach(args)?;
            }
        }
        Ok(())
    }

    pub(super) fn handle_disconnect(&mut self, args: Option<DisconnectArguments>) -> Result<(), Error> {
        if let Some(commands) = &self.pre_terminate_commands {
            self.exec_commands("preTerminateCommands", &commands)?;
        }

        // Let go of the terminal helper connection
        self.debuggee_terminal = None;
        self.terminate_debuggee(args.map(|a| a.terminate_debuggee).flatten())?;

        if let Some(commands) = &self.exit_commands {
            self.exec_commands("exitCommands", &commands)?;
        }

        Ok(())
    }

    fn terminate_debuggee(&mut self, force_terminate: Option<bool>) -> Result<(), Error> {
        let process = self.target.process();
        if process.is_valid() {
            let state = process.state();
            if state.is_alive() {
                let terminate = force_terminate.unwrap_or(self.terminate_on_disconnect);
                if terminate {
                    process.kill()?;
                } else {
                    process.detach(false)?;
                }
            }
        }
        Ok(())
    }

    fn create_target_from_program(&self, program: &str) -> Result<SBTarget, Error> {
        match self.debugger.create_target(Some(Path::new(program)), None, None, false) {
            Ok(target) => Ok(target),
            Err(err) => {
                // TODO: use selected platform instead of cfg!(windows)
                if cfg!(windows) && !program.ends_with(".exe") {
                    let program = format!("{}.exe", program);
                    self.debugger.create_target(Some(Path::new(&program)), None, None, false)
                } else {
                    Err(err)
                }
            }
        }
        .map_err(|err| blame_user(err.into()).into())
    }

    fn set_target(&mut self, target: SBTarget) {
        self.debugger.listener().stop_listening_for_events(&self.target.broadcaster(), !0);
        self.target = target;
        self.disasm_ranges = disassembly::DisassembledRanges::new(&self.target);
        self.debugger.listener().start_listening_for_events(&self.target.broadcaster(), !0);
    }

    // Try to create a debuggee terminal, according to what was requested in the launch configuration.
    // On success, initializes DebugSession::debuggee_terminal.
    fn create_terminal(&mut self, args: &LaunchRequestArguments) -> impl Future {
        if self.target.platform().name() != "host" {
            return future::ready(()).left_future(); // Can't attach to a terminal when remote-debugging.
        }
        if !self.client_caps.supports_run_in_terminal_request.unwrap_or(false) {
            return future::ready(()).left_future(); // The client doesn't support "runInTerminal" request.
        }

        let terminal_kind = match args.terminal {
            Some(kind) => kind,
            None => match args.console {
                Some(ConsoleKind::InternalConsole) => TerminalKind::Console,
                Some(ConsoleKind::ExternalTerminal) => TerminalKind::External,
                Some(ConsoleKind::IntegratedTerminal) => TerminalKind::Integrated,
                None => TerminalKind::Integrated,
            },
        };
        let terminal_kind = match terminal_kind {
            TerminalKind::Console => return future::ready(()).left_future(),
            TerminalKind::External => "external",
            TerminalKind::Integrated => "integrated",
        };

        let title = args.common.name.as_deref().unwrap_or("Debug").to_string();
        let fut = Terminal::create(
            terminal_kind,
            title,
            self.dap_session.clone(),
        );
        let self_ref = self.self_ref.clone();
        async move {
            let result = fut.await;
            self_ref
                .map(|s| match result {
                    Ok(terminal) => s.debuggee_terminal = Some(terminal),
                    Err(err) => s.console_error(format!(
                        "Failed to redirect stdio to a terminal. ({})\nDebuggee output will appear here.",
                        err
                    )),
                })
                .await
        }
        .right_future()
    }

    fn configure_stdio(&mut self, args: &LaunchRequestArguments, launch_info: &mut SBLaunchInfo) -> Result<(), Error> {
        let mut stdio = match args.stdio {
            None => vec![],
            Some(Either::First(ref stdio)) => vec![Some(stdio.clone())], // A single string
            Some(Either::Second(ref stdio)) => stdio.clone(),            // List of strings
        };
        // Pad to at least 3 entries
        while stdio.len() < 3 {
            stdio.push(None)
        }

        if let Some(terminal) = &self.debuggee_terminal {
            for (fd, name) in stdio.iter().enumerate() {
                // Use file name specified in the launch config if available,
                // otherwise use the appropriate terminal device name.
                let name = match (name, fd) {
                    (Some(name), _) => name,
                    (None, 0) => terminal.input_devname(),
                    (None, _) => terminal.output_devname(),
                };
                let _ = match fd {
                    0 => launch_info.add_open_file_action(fd as i32, Path::new(name), true, false),
                    1 => launch_info.add_open_file_action(fd as i32, Path::new(name), false, true),
                    2 => launch_info.add_open_file_action(fd as i32, Path::new(name), false, true),
                    _ => launch_info.add_open_file_action(fd as i32, Path::new(name), true, true),
                };
            }
        }

        Ok(())
    }

    // Handle initialization tasks common to both launching and attaching
    fn common_init_session(&mut self, args_common: &CommonLaunchFields) -> Result<(), Error> {
        if let Some(expressions) = args_common.expressions {
            self.default_expr_type = expressions;
        }
        if let None = self.python {
            match self.default_expr_type {
                Expressions::Simple | Expressions::Python => self.console_error(
                    "Could not initialize Python interpreter - some features will be unavailable (e.g. debug visualizers).",
                ),
                Expressions::Native => (),
            }
            self.default_expr_type = Expressions::Native;
        }

        if let Some(source_map) = &args_common.source_map {
            self.init_source_map(source_map.iter().map(|(k, v)| (k, v.as_ref())));
        }

        self.relative_path_base = Initialized(match &args_common.relative_path_base {
            Some(base) => base.into(),
            None => env::current_dir()?,
        });

        if let Some(true) = &args_common.reverse_debugging {
            self.send_event(EventBody::capabilities(CapabilitiesEventBody {
                capabilities: Capabilities {
                    supports_step_back: Some(true),
                    ..Default::default()
                },
            }));
        }

        if let Some(breakpoint_mode) = &args_common.breakpoint_mode {
            self.breakpoint_mode = breakpoint_mode.to_owned();
        }

        if let Some(ref settings) = args_common.adapter_settings {
            self.update_adapter_settings_and_caps(settings);
        }

        // Normally, source_langiuages is set in adapter settings we get on the command line,
        // but for the sake of people who use bare adapter, we also allow setting it via launch config.
        if let Some(ref source_languages) = args_common.source_languages {
            self.update_adapter_settings_and_caps(&AdapterSettings {
                source_languages: Some(source_languages.clone()),
                ..Default::default()
            });
        }

        self.print_console_mode();

        if let Some(commands) = &args_common.init_commands {
            self.exec_commands("initCommands", &commands)?;
        }

        if let Some(python) = &self.python {
            log_errors!(python.init_lang_support());
        }

        if let Some(settings) = &args_common.adapter_settings {
            if let Some(ConsoleMode::Split) = settings.console_mode {
                let name = args_common.name.as_deref().unwrap_or("Debug");
                self.create_debugger_terminal(name);
            } else {
                // Witoout a terminal, confirmations will just hang the session.
                self.debugger.set_variable("auto-confirm", "true")?;
            }
        }

        Ok(())
    }

    fn common_post_run(&mut self, args_common: CommonLaunchFields) -> Result<(), Error> {
        if let Some(commands) = args_common.post_run_commands {
            log_errors!(self.exec_commands("postRunCommands", &commands));
        }
        self.pre_terminate_commands = args_common.pre_terminate_commands;
        self.exit_commands = args_common.exit_commands;

        Ok(())
    }

    pub(super) fn print_console_mode(&self) {
        let message = match self.console_mode {
            ConsoleMode::Commands => "Console is in 'commands' mode, prefix expressions with '?'.",
            ConsoleMode::Split | ConsoleMode::Evaluate => {
                "Console is in 'evaluation' mode, prefix commands with '/cmd ' or '`'."
            }
        };
        self.console_message(message);
    }

    fn init_source_map<S: AsRef<str>>(&mut self, source_map: impl IntoIterator<Item = (S, Option<S>)>) {
        let mut args = String::new();
        for (remote, local) in source_map.into_iter() {
            let remote_escaped = lldb_quoted_string(remote.as_ref());
            let local_escaped = lldb_quoted_string(local.as_ref().map_or("", AsRef::as_ref));
            args.push_str(&remote_escaped);
            args.push_str(" ");
            args.push_str(&local_escaped);
            args.push_str(" ");
        }

        if !args.is_empty() {
            info!("Set target.source-map args: {}", args);
            if let Err(error) = self.debugger.set_variable("target.source-map", &args) {
                self.console_error(format!("Could not set source map: {}", error.error_string()))
            }
        }
    }
}

// Create a functor that performs folding of environment variable keys differing only by case
fn make_env_key_folder(ignore_case: bool) -> impl FnMut(&str) -> String {
    use std::collections::hash_map::Entry;
    use std::collections::HashMap;

    let mut case_map: HashMap<String, String> = HashMap::new();
    move |k: &str| {
        if ignore_case {
            let uk = k.to_uppercase();
            match case_map.entry(uk) {
                Entry::Occupied(e) => e.get().into(),
                Entry::Vacant(e) => {
                    e.insert(k.into());
                    k.into()
                }
            }
        } else {
            k.into()
        }
    }
}

#[test]
fn env_key_transform() {
    let mut folder = make_env_key_folder(true);
    assert_eq!(folder("Path"), "Path");
    assert_eq!(folder("PATH"), "Path");

    assert_eq!(folder("Foo"), "Foo");
    assert_eq!(folder("foo"), "Foo");
}
