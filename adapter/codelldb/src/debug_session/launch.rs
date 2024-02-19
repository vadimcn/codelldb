use crate::prelude::*;

use crate::must_initialize::Initialized;

use super::*;
use adapter_protocol::*;
use lldb::*;

impl super::DebugSession {
    pub fn report_launch_cfg_error(&mut self, err: serde_json::Error) -> Result<ResponseBody, Error> {
        bail!(as_user_error(format!("Could not parse launch configuration: {}", err)))
    }

    pub fn handle_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        self.common_init_session(&args.common)?;

        if let Some(true) = &args.custom {
            self.handle_custom_launch(args)
        } else {
            self.no_debug = args.no_debug.unwrap_or(false);

            let target = if let Some(commands) = &args.target_create_commands {
                self.exec_commands("targetCreateCommands", &commands)?;
                self.debugger.selected_target()
            } else {
                let program = match &args.program {
                    Some(program) => program,
                    None => bail!(as_user_error("The \"program\" attribute is required for launch.")),
                };
                self.create_target_from_program(program)?
            };
            self.target = Initialized(target);
            self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));
            self.send_event(EventBody::initialized);

            let term_fut = self.create_terminal(&args);
            let mut config_done_recv = self.configuration_done_sender.subscribe();
            let self_ref = self.self_ref.clone();
            let fut = async move {
                term_fut.await;
                log_errors!(config_done_recv.recv().await);
                self_ref.map(|s| s.complete_launch(args)).await
            };
            Err(AsyncResponse(Box::new(fut)).into())
        }
    }

    fn complete_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        let mut launch_info = self.target.launch_info();

        let mut launch_env: HashMap<String, String> = HashMap::new();
        let mut fold_case = make_case_folder();

        let inherit_env = match self.debugger.get_variable("target.inherit-env").string_at_index(0) {
            Some("true") => true,
            _ => false,
        };
        // Init with host environment if `inherit-env` is set.
        if inherit_env {
            for (k, v) in env::vars() {
                launch_env.insert(fold_case(&k), v);
            }
        }
        if let Some(ref env_file) = args.env_file {
            for item in dotenvy::from_filename_iter(env_file)? {
                let (k, v) = item?;
                launch_env.insert(fold_case(&k), v);
            }
        }
        if let Some(ref env) = args.env {
            for (k, v) in env.iter() {
                launch_env.insert(fold_case(k), v.into());
            }
        }
        let launch_env = launch_env.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<String>>();
        launch_info.set_environment_entries(launch_env.iter().map(|s| s.as_ref()), false);

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

        let do_launch = || -> Result<SBProcess, Error> {
            if let Some(commands) = &args.process_create_commands {
                self.exec_commands("processCreateCommands", commands)?;
                Ok(self.target.process())
            } else {
                self.target.launch(&launch_info).map_err(|sberr| Box::new(sberr).into())
            }
        };

        let result = match &self.debuggee_terminal {
            Some(t) => {
                // Windows does not have an API for launching child process in a different console than the parent
                // process, so we have to attach adapter to the target console before launching.
                #[cfg(windows)]
                t.attach_console();
                let result = do_launch();
                #[cfg(windows)]
                t.detach_console();
                drop(t);
                result
            }
            None => do_launch(),
        };

        let process = match result {
            Ok(process) => process,
            Err(err) => {
                let mut msg = err.to_string();
                if let Some(work_dir) = launch_info.working_directory() {
                    if self.target.platform().get_file_permissions(work_dir) == 0 {
                        #[rustfmt::skip]
                        log_errors!(write!(msg,
                            "\n\nPossible cause: the working directory \"{}\" is missing or inaccessible.",
                            work_dir.display()
                        ));
                    }
                }
                bail!(as_user_error(msg))
            }
        };
        self.console_message(format!("Launched process {}", process.process_id()));
        self.terminate_on_disconnect = true;

        // LLDB sometimes loses the initial stop event.
        if launch_info.launch_flags().intersects(LaunchFlag::StopAtEntry) {
            self.notify_process_stopped();
        }

        self.common_post_run(args.common)?;

        Ok(ResponseBody::launch)
    }

    fn handle_custom_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        if let Some(commands) = &args.target_create_commands {
            self.exec_commands("targetCreateCommands", &commands)?;
        }
        self.target = Initialized(self.debugger.selected_target());
        self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));
        self.send_event(EventBody::initialized);

        let mut config_done_recv = self.configuration_done_sender.subscribe();
        let self_ref = self.self_ref.clone();
        let fut = async move {
            log_errors!(config_done_recv.recv().await);
            self_ref.map(|s| s.complete_custom_launch(args)).await
        };
        Err(AsyncResponse(Box::new(fut)).into())
    }

    fn complete_custom_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        if let Some(commands) = args.process_create_commands.as_ref().or(args.common.pre_run_commands.as_ref()) {
            self.exec_commands("processCreateCommands", &commands)?;
        }
        self.terminate_on_disconnect = true;

        // This is susceptible to race conditions, but probably the best we can do.
        if self.target.process().state().is_stopped() {
            self.notify_process_stopped();
        }

        self.common_post_run(args.common)?;

        Ok(ResponseBody::launch)
    }

    pub fn handle_attach(&mut self, args: AttachRequestArguments) -> Result<ResponseBody, Error> {
        self.common_init_session(&args.common)?;

        if args.program.is_none() && args.pid.is_none() && args.target_create_commands.is_none() {
            bail!(as_user_error(r#"Either "program" or "pid" is required for attach."#));
        }

        let target = if let Some(commands) = &args.target_create_commands {
            self.exec_commands("targetCreateCommands", &commands)?;
            self.debugger.selected_target()
        } else {
            // Try to create target from `program`.
            let target = if let Some(program) = &args.program {
                match self.create_target_from_program(program) {
                    Ok(target) => Some(target),
                    Err(_) => None,
                }
            } else {
                None
            };
            // Fall back to a dummy target.
            match target {
                Some(target) => target,
                None => self.debugger.create_target(Path::new(""), None, None, false)?,
            }
        };
        self.target = Initialized(target);
        self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));

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

        let process = if let Some(commands) = &args.process_create_commands {
            self.exec_commands("processCreateCommands", commands)?;
            self.target.process()
        } else {
            let attach_info = SBAttachInfo::new();
            if let Some(pid) = &args.pid {
                let pid = match pid {
                    Pid::Number(n) => *n as ProcessID,
                    Pid::String(s) => match s.parse() {
                        Ok(n) => n,
                        Err(_) => bail!(as_user_error("Process id must be a positive integer.")),
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

            match self.target.attach(&attach_info) {
                Ok(process) => process,
                Err(err) => bail!(as_user_error(format!("Could not attach: {}", err))),
            }
        };
        self.console_message(format!("Attached to process {}", process.process_id()));
        self.terminate_on_disconnect = false;

        if args.common.stop_on_entry.unwrap_or(false) {
            self.notify_process_stopped();
        } else {
            log_errors!(self.target.process().resume());
        }

        self.common_post_run(args.common)?;

        Ok(ResponseBody::attach)
    }

    fn create_target_from_program(&self, program: &str) -> Result<SBTarget, Error> {
        match self.debugger.create_target(Path::new(program), None, None, false) {
            Ok(target) => Ok(target),
            Err(err) => {
                // TODO: use selected platform instead of cfg!(windows)
                if cfg!(windows) && !program.ends_with(".exe") {
                    let program = format!("{}.exe", program);
                    self.debugger.create_target(Path::new(&program), None, None, false)
                } else {
                    Err(err)
                }
            }
        }
        .map_err(|e| as_user_error(e).into())
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
            self.terminal_prompt_clear.clone(),
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
                    0 => launch_info.add_open_file_action(fd as i32, name, true, false),
                    1 => launch_info.add_open_file_action(fd as i32, name, false, true),
                    2 => launch_info.add_open_file_action(fd as i32, name, false, true),
                    _ => launch_info.add_open_file_action(fd as i32, name, true, true),
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

        self.print_console_mode();

        if cfg!(unix) {
            let commands = ["process handle SIGINT -p true -s false".to_string()];
            self.exec_commands("SIGINT passthrough for graceful terminate request", &commands)?;
        }

        if let Some(commands) = &args_common.init_commands {
            self.exec_commands("initCommands", &commands)?;
        }

        if let Some(python) = &self.python {
            log_errors!(python.load_formatters());
        }

        Ok(())
    }

    fn common_post_run(&mut self, args_common: CommonLaunchFields) -> Result<(), Error> {
        if let Some(commands) = args_common.post_run_commands {
            self.exec_commands("postRunCommands", &commands)?;
        }
        self.exit_commands = args_common.exit_commands;

        if let Some(settings) = args_common.adapter_settings {
            if let Some(ConsoleMode::Split) = settings.console_mode {
                let name = args_common.name.as_deref().unwrap_or("Debug");
                self.create_debugger_terminal(name);
            }
        }

        Ok(())
    }

    pub fn print_console_mode(&self) {
        let message = match self.console_mode {
            ConsoleMode::Commands => "Console is in 'commands' mode, prefix expressions with '?'.",
            ConsoleMode::Split | ConsoleMode::Evaluate => {
                "Console is in 'evaluation' mode, prefix commands with '/cmd ' or '`'."
            }
        };
        self.console_message(message);
    }

    fn init_source_map<S>(&mut self, source_map: impl IntoIterator<Item = (S, Option<S>)>)
    where
        S: AsRef<str>,
    {
        fn escape(s: &str) -> String {
            s.replace("\\", "\\\\").replace("\"", "\\\"")
        }

        let mut args = String::new();
        for (remote, local) in source_map.into_iter() {
            let remote_escaped = escape(remote.as_ref());
            let local_escaped = match local {
                None => String::new(),
                Some(s) => escape(s.as_ref()),
            };
            args.push_str("\"");
            args.push_str(&remote_escaped);
            args.push_str("\" \"");
            args.push_str(&local_escaped);
            args.push_str("\" ");
        }

        if !args.is_empty() {
            info!("Set target.source-map args: {}", args);
            if let Err(error) = self.debugger.set_variable("target.source-map", &args) {
                self.console_error(format!("Could not set source map: {}", error.error_string()))
            }
        }
    }
}
