mod breakpoints;
mod variables;

use crate::prelude::*;

use crate::cancellation;
use crate::dap_session::DAPSession;
use crate::debug_event_listener;
use crate::disassembly;
use crate::fsutil::normalize_path;
use crate::handles::{self, HandleTree};
use crate::must_initialize::{Initialized, MustInitialize, NotInitialized};
use crate::platform::{get_fs_path_case, make_case_folder, pipe};
use crate::python::{self, PythonInterface};
use crate::shared::Shared;
use crate::terminal::Terminal;
use breakpoints::BreakpointsState;
use variables::Container;

use std;
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp;
use std::collections::HashMap;
use std::env;
use std::ffi::CStr;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str;
use std::time;

use adapter_protocol::*;
use futures;
use futures::prelude::*;
use lldb::*;
use serde_json;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

pub struct DebugSession {
    self_ref: MustInitialize<Shared<DebugSession>>,
    dap_session: DAPSession,
    event_listener: SBListener,
    python: Option<Box<PythonInterface>>,
    current_cancellation: cancellation::Receiver, // Cancellation associated with request currently being processed

    console_pipe: std::fs::File,

    debugger: SBDebugger,
    target: MustInitialize<SBTarget>,
    process: MustInitialize<SBProcess>,
    terminate_on_disconnect: bool,
    no_debug: bool,

    breakpoints: RefCell<BreakpointsState>,
    var_refs: HandleTree<Container>,
    disassembly: MustInitialize<disassembly::AddressSpace>,
    source_map_cache: RefCell<HashMap<PathBuf, Option<Rc<PathBuf>>>>,
    loaded_modules: Vec<SBModule>,
    relative_path_base: MustInitialize<PathBuf>,
    exit_commands: Option<Vec<String>>,
    debuggee_terminal: Option<Terminal>,
    selected_frame_changed: bool,
    last_goto_request: Option<GotoTargetsArguments>,

    client_caps: MustInitialize<InitializeRequestArguments>,

    default_expr_type: Expressions,
    global_format: Format,
    show_disassembly: ShowDisassembly,
    deref_pointers: bool,
    console_mode: ConsoleMode,
    suppress_missing_files: bool,
    evaluate_for_hovers: bool,
    command_completions: bool,
    evaluation_timeout: time::Duration,
    source_languages: Vec<String>,
    terminal_prompt_clear: Option<Vec<String>>,
}

// AsyncResponse is used to "smuggle" futures out of request handlers
// in the few cases when we need to respond asynchronously.
struct AsyncResponse(pub Box<dyn Future<Output = Result<ResponseBody, Error>> + 'static>);

impl std::error::Error for AsyncResponse {}
impl std::fmt::Debug for AsyncResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "AsyncResponse")
    }
}
impl std::fmt::Display for AsyncResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "AsyncResponse")
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////

unsafe impl Send for DebugSession {}

impl DebugSession {
    pub fn run(dap_session: DAPSession, settings: AdapterSettings) -> impl Future {
        let debugger = SBDebugger::create(false);
        debugger.set_async_mode(true);

        let (con_reader, con_writer) = pipe().unwrap();
        log_errors!(debugger.set_output_stream(con_writer.try_clone().unwrap()));
        let current_exe = env::current_exe().unwrap();

        let (python, python_events) = match python::initialize(
            debugger.command_interpreter(),
            current_exe.parent().unwrap(),
            Some(con_writer.try_clone().unwrap()),
        ) {
            Ok((python, events)) => (Some(python), Some(events)),
            Err(err) => {
                error!("Initialize Python interpreter: {}", err);
                (None, None)
            }
        };

        let con_reader = tokio::fs::File::from_std(con_reader);

        let mut debug_session = DebugSession {
            self_ref: NotInitialized,
            dap_session: dap_session,
            event_listener: SBListener::new_with_name("DebugSession"),
            python: python,
            current_cancellation: cancellation::dummy(),

            console_pipe: con_writer,

            debugger: debugger,
            target: NotInitialized,
            process: NotInitialized,
            terminate_on_disconnect: false,
            no_debug: false,

            breakpoints: RefCell::new(BreakpointsState::new()),
            var_refs: HandleTree::new(),
            disassembly: NotInitialized,
            source_map_cache: RefCell::new(HashMap::new()),
            loaded_modules: Vec::new(),
            relative_path_base: NotInitialized,
            exit_commands: None,
            debuggee_terminal: None,
            selected_frame_changed: false,
            last_goto_request: None,

            client_caps: NotInitialized,

            default_expr_type: Expressions::Simple,
            global_format: Format::Default,
            show_disassembly: ShowDisassembly::Auto,
            deref_pointers: true,
            console_mode: ConsoleMode::Commands,
            suppress_missing_files: true,
            evaluate_for_hovers: true,
            command_completions: true,
            evaluation_timeout: time::Duration::from_secs(5),
            source_languages: vec!["cpp".into()],
            terminal_prompt_clear: None,
        };

        DebugSession::pipe_console_events(&debug_session.dap_session, con_reader);

        if let Some(python_events) = python_events {
            DebugSession::pipe_python_events(&debug_session.dap_session, python_events);
        }

        let mut requests_receiver = DebugSession::cancellation_filter(&debug_session.dap_session.clone());
        let mut debug_events_stream = debug_event_listener::start_polling(&debug_session.event_listener);

        debug_session.update_adapter_settings(&settings);

        // The main event loop, where we react to incoming events from different sources.
        let shared_session = Shared::new(debug_session);
        shared_session.try_map(|s| s.self_ref = Initialized(shared_session.clone())).unwrap();

        let local_set = tokio::task::LocalSet::new();
        local_set.spawn_local(async move {
            loop {
                tokio::select! {
                    // Requests from VSCode
                    request = requests_receiver.recv() => {
                        match request {
                            Some((seq, request, cancellation)) => shared_session.map(
                                    |s| s.handle_request(seq, request, cancellation)).await,
                            None => {
                                debug!("End of the requests stream");
                                break;
                            }
                        }
                    }
                    // LLDB events.
                    Some(event) = debug_events_stream.recv() => {
                        shared_session.map( |s| s.handle_debug_event(event)).await;
                    }
                }
            }
            SBBreakpoint::clear_all_callbacks(); // Callbacks hold references to the session
            shared_session.map(|s| s.self_ref = NotInitialized).await;
            if shared_session.ref_count() > 1 {
                error!("shared_session.ref_count={}", shared_session.ref_count());
            }
        });
        local_set
    }

    fn pipe_console_events(dap_session: &DAPSession, mut con_reader: tokio::fs::File) {
        let dap_session = dap_session.clone();
        tokio::spawn(async move {
            let mut con_data = [0u8; 1024];
            loop {
                if let Ok(bytes) = con_reader.read(&mut con_data).await {
                    let event = EventBody::output(OutputEventBody {
                        output: String::from_utf8_lossy(&con_data[0..bytes]).into(),
                        ..Default::default()
                    });
                    log_errors!(dap_session.send_event(event).await);
                }
            }
        });
    }

    fn pipe_python_events(dap_session: &DAPSession, mut python_events: mpsc::Receiver<EventBody>) {
        let dap_session = dap_session.clone();
        tokio::spawn(async move {
            while let Some(event) = python_events.recv().await {
                log_errors!(dap_session.send_event(event).await);
            }
        });
    }

    /// Handle request cancellations.
    fn cancellation_filter(
        dap_session: &DAPSession,
    ) -> mpsc::Receiver<(u32, RequestArguments, cancellation::Receiver)> {
        use tokio::sync::broadcast::error::RecvError;

        let mut raw_requests_stream = dap_session.subscribe_requests().unwrap();
        let (requests_sender, requests_receiver) =
            mpsc::channel::<(u32, RequestArguments, cancellation::Receiver)>(100);

        // This task pairs incoming requests with a cancellation token, which is activated upon receiving a "cancel" request.
        let filter = async move {
            let mut pending_requests: HashMap<u32, cancellation::Sender> = HashMap::new();
            let mut cancellable_requests: Vec<cancellation::Sender> = Vec::new();

            loop {
                match raw_requests_stream.recv().await {
                    Ok((seq, request)) => {
                        let sender = cancellation::Sender::new();
                        let receiver = sender.subscribe();

                        match request {
                            RequestArguments::cancel(args) => {
                                info!("Cancellation {:?}", args);
                                if let Some(id) = args.request_id {
                                    if let Some(sender) = pending_requests.remove(&(id as u32)) {
                                        sender.send();
                                    }
                                }
                                continue; // Dont forward to the main event loop.
                            }
                            // Requests that may be canceled.
                            RequestArguments::scopes(_)
                            | RequestArguments::variables(_)
                            | RequestArguments::evaluate(_) => cancellable_requests.push(sender.clone()),
                            // Requests that will cancel the above.
                            RequestArguments::continue_(_)
                            | RequestArguments::pause(_)
                            | RequestArguments::next(_)
                            | RequestArguments::stepIn(_)
                            | RequestArguments::stepOut(_)
                            | RequestArguments::stepBack(_)
                            | RequestArguments::reverseContinue(_)
                            | RequestArguments::terminate(_)
                            | RequestArguments::disconnect(_) => {
                                for sender in &mut cancellable_requests {
                                    sender.send();
                                }
                            }
                            _ => (),
                        }

                        pending_requests.insert(seq, sender);
                        log_errors!(requests_sender.send((seq, request, receiver)).await);

                        // Clean out entries which don't have any receivers.
                        pending_requests.retain(|_k, v| v.receiver_count() > 0);
                        cancellable_requests.retain(|v| v.receiver_count() > 0);
                    }
                    Err(RecvError::Lagged(count)) => error!("Missed {} messages", count),
                    Err(RecvError::Closed) => break,
                }
            }
        };
        tokio::spawn(filter);

        requests_receiver
    }

    fn handle_request(&mut self, seq: u32, request_args: RequestArguments, cancellation: cancellation::Receiver) {
        if cancellation.is_cancelled() {
            self.send_response(seq, Err("canceled".into()));
        } else {
            self.current_cancellation = cancellation;
            if let RequestArguments::unknown = request_args {
                info!("Received an unknown command");
                self.send_response(seq, Err("Not implemented.".into()));
            } else {
                let result = self.handle_request_args(request_args);
                self.current_cancellation = cancellation::dummy();
                match result {
                    // Spawn async responses as tasks
                    Err(err) if err.is::<AsyncResponse>() => {
                        let self_ref = self.self_ref.clone();
                        tokio::task::spawn_local(async move {
                            let fut: std::pin::Pin<Box<_>> = err.downcast::<AsyncResponse>().unwrap().0.into();
                            let result = fut.await;
                            self_ref.map(|s| s.send_response(seq, result)).await;
                        });
                    }
                    // Send synchronous results immediately
                    _ => {
                        self.send_response(seq, result);
                    }
                }
            }
        }
    }

    #[rustfmt::skip]
    fn handle_request_args(&mut self, arguments: RequestArguments) -> Result<ResponseBody, Error> {
        match arguments {
            RequestArguments::_adapterSettings(args) =>
                self.handle_adapter_settings(args)
                    .map(|_| ResponseBody::_adapterSettings),
            RequestArguments::initialize(args) =>
                self.handle_initialize(args)
                    .map(|r| ResponseBody::initialize(r)),
            RequestArguments::launch(Either::First(args)) =>
                    self.handle_launch(args),
            RequestArguments::launch(Either::Second(args)) =>
                    self.report_launch_cfg_error(serde_json::from_value::<LaunchRequestArguments>(args).unwrap_err()),
            RequestArguments::attach(Either::First(args)) =>
                    self.handle_attach(args),
            RequestArguments::attach(Either::Second(args)) =>
                    self.report_launch_cfg_error(serde_json::from_value::<AttachRequestArguments>(args).unwrap_err()),
            RequestArguments::configurationDone(_) =>
                self.handle_configuration_done()
                    .map(|_| ResponseBody::configurationDone),
            RequestArguments::disconnect(args) =>
                self.handle_disconnect(args)
                    .map(|_| ResponseBody::disconnect),
            _ => {
                if self.no_debug {
                    bail!("Not supported in noDebug mode.")
                } else {
                    match arguments {
                        RequestArguments::setBreakpoints(args) =>
                            self.handle_set_breakpoints(args)
                                .map(|r| ResponseBody::setBreakpoints(r)),
                        RequestArguments::setFunctionBreakpoints(args) =>
                            self.handle_set_function_breakpoints(args)
                                .map(|r| ResponseBody::setFunctionBreakpoints(r)),
                        RequestArguments::setExceptionBreakpoints(args) =>
                            self.handle_set_exception_breakpoints(args)
                                .map(|_| ResponseBody::setExceptionBreakpoints),
                        RequestArguments::threads(_) =>
                            self.handle_threads()
                                .map(|r| ResponseBody::threads(r)),
                        RequestArguments::stackTrace(args) =>
                            self.handle_stack_trace(args)
                                .map(|r| ResponseBody::stackTrace(r)),
                        RequestArguments::scopes(args) =>
                            self.handle_scopes(args)
                                .map(|r| ResponseBody::scopes(r)),
                        RequestArguments::variables(args) =>
                            self.handle_variables(args)
                                .map(|r| ResponseBody::variables(r)),
                        RequestArguments::evaluate(args) =>
                            self.handle_evaluate(args),
                        RequestArguments::setVariable(args) =>
                            self.handle_set_variable(args)
                                .map(|r| ResponseBody::setVariable(r)),
                        RequestArguments::pause(args) =>
                            self.handle_pause(args)
                                .map(|_| ResponseBody::pause),
                        RequestArguments::continue_(args) =>
                            self.handle_continue(args)
                                .map(|r| ResponseBody::continue_(r)),
                        RequestArguments::next(args) =>
                            self.handle_next(args)
                                .map(|_| ResponseBody::next),
                        RequestArguments::stepIn(args) =>
                            self.handle_step_in(args)
                                .map(|_| ResponseBody::stepIn),
                        RequestArguments::stepOut(args) =>
                            self.handle_step_out(args)
                                .map(|_| ResponseBody::stepOut),
                        RequestArguments::stepBack(args) =>
                            self.handle_step_back(args)
                                .map(|_| ResponseBody::stepBack),
                        RequestArguments::reverseContinue(args) =>
                            self.handle_reverse_continue(args)
                                .map(|_| ResponseBody::reverseContinue),
                        RequestArguments::source(args) =>
                            self.handle_source(args)
                                .map(|r| ResponseBody::source(r)),
                        RequestArguments::completions(args) =>
                            self.handle_completions(args)
                                .map(|r| ResponseBody::completions(r)),
                        RequestArguments::gotoTargets(args) =>
                            self.handle_goto_targets(args)
                                .map(|r| ResponseBody::gotoTargets(r)),
                        RequestArguments::goto(args) =>
                            self.handle_goto(args)
                                .map(|_| ResponseBody::goto),
                        RequestArguments::restartFrame(args) =>
                            self.handle_restart_frame(args)
                                .map(|_| ResponseBody::restartFrame),
                        RequestArguments::dataBreakpointInfo(args) =>
                            self.handle_data_breakpoint_info(args)
                                .map(|r| ResponseBody::dataBreakpointInfo(r)),
                        RequestArguments::setDataBreakpoints(args) =>
                            self.handle_set_data_breakpoints(args)
                                .map(|r| ResponseBody::setDataBreakpoints(r)),
                        RequestArguments::readMemory(args) =>
                            self.handle_read_memory(args)
                                .map(|r| ResponseBody::readMemory(r)),
                        RequestArguments::writeMemory(args) =>
                            self.handle_write_memory(args)
                                .map(|r| ResponseBody::writeMemory(r)),
                        RequestArguments::_symbols(args) =>
                            self.handle_symbols(args)
                                .map(|r| ResponseBody::_symbols(r)),
                        _=> bail!("Not implemented.")
                    }
                }
            }
        }
    }

    fn send_response(&self, request_seq: u32, result: Result<ResponseBody, Error>) {
        let response = match result {
            Ok(body) => Response {
                request_seq: request_seq,
                success: true,
                result: ResponseResult::Success {
                    body: body,
                },
            },
            Err(err) => {
                let message = if let Some(user_err) = err.downcast_ref::<crate::error::UserError>() {
                    format!("{}", user_err)
                } else {
                    format!("Internal debugger error: {}", err)
                };
                error!("{}", message);
                Response {
                    request_seq: request_seq,
                    success: false,
                    result: ResponseResult::Error {
                        command: "".into(),
                        message: message,
                        show_user: Some(true),
                    },
                }
            }
        };
        log_errors!(self.dap_session.try_send_response(response));
    }

    fn send_event(&self, event_body: EventBody) {
        log_errors!(self.dap_session.try_send_event(event_body));
    }

    fn console_message(&self, output: impl std::fmt::Display) {
        self.console_message_nonl(format!("{}\n", output));
    }

    fn console_message_nonl(&self, output: impl std::fmt::Display) {
        self.send_event(EventBody::output(OutputEventBody {
            output: format!("{}", output),
            ..Default::default()
        }));
    }

    fn console_error(&self, output: impl std::fmt::Display) {
        self.send_event(EventBody::output(OutputEventBody {
            output: format!("{}\n", output),
            category: Some("stderr".into()),
            ..Default::default()
        }));
    }

    fn handle_initialize(&mut self, args: InitializeRequestArguments) -> Result<Capabilities, Error> {
        self.event_listener.start_listening_for_event_class(&self.debugger, SBTarget::broadcaster_class_name(), !0);
        self.event_listener.start_listening_for_event_class(&self.debugger, SBProcess::broadcaster_class_name(), !0);
        self.event_listener.start_listening_for_event_class(&self.debugger, SBThread::broadcaster_class_name(), !0);
        self.client_caps = Initialized(args);
        Ok(self.make_capabilities())
    }

    fn make_capabilities(&self) -> Capabilities {
        Capabilities {
            supports_configuration_done_request: Some(true),
            supports_function_breakpoints: Some(true),
            supports_conditional_breakpoints: Some(true),
            supports_hit_conditional_breakpoints: Some(true),
            supports_set_variable: Some(true),
            supports_goto_targets_request: Some(true),
            supports_delayed_stack_trace_loading: Some(true),
            support_terminate_debuggee: Some(true),
            supports_log_points: Some(true),
            supports_data_breakpoints: Some(true),
            supports_restart_frame: Some(true),
            supports_cancel_request: Some(true),
            supports_read_memory_request: Some(true),
            supports_write_memory_request: Some(true),
            supports_evaluate_for_hovers: Some(self.evaluate_for_hovers),
            supports_completions_request: Some(self.command_completions),
            exception_breakpoint_filters: Some(self.get_exception_filters(&self.source_languages)),
            ..Default::default()
        }
    }

    fn get_exception_filters(&self, source_langs: &[String]) -> Vec<ExceptionBreakpointsFilter> {
        let mut filters = vec![];
        if source_langs.iter().any(|x| x == "cpp") {
            filters.push(ExceptionBreakpointsFilter {
                filter: "cpp_throw".into(),
                label: "C++: on throw".into(),
                default: Some(true),
                ..Default::default()
            });
            filters.push(ExceptionBreakpointsFilter {
                filter: "cpp_catch".into(),
                label: "C++: on catch".into(),
                default: Some(false),
                ..Default::default()
            });
        }
        if source_langs.iter().any(|x| x == "rust") {
            filters.push(ExceptionBreakpointsFilter {
                filter: "rust_panic".into(),
                label: "Rust: on panic".into(),
                default: Some(true),
                ..Default::default()
            });
        }
        filters
    }

    fn report_launch_cfg_error(&mut self, err: serde_json::Error) -> Result<ResponseBody, Error> {
        bail!(as_user_error(format!("Could not parse launch configuration: {}", err)))
    }

    fn handle_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        self.common_init_session(&args.common)?;

        if let Some(true) = &args.custom {
            self.handle_custom_launch(args)
        } else {
            let program = match &args.program {
                Some(program) => program,
                None => bail!(as_user_error("\"program\" property is required for launch")),
            };

            self.no_debug = args.no_debug.unwrap_or(false);
            self.target = Initialized(self.create_target_from_program(program)?);
            self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));
            self.send_event(EventBody::initialized);

            let term_fut = self.create_terminal(&args);
            let config_done_fut = self.wait_for_configuration_done();
            let self_ref = self.self_ref.clone();
            let fut = async move {
                drop(tokio::join!(term_fut, config_done_fut));
                self_ref.map(|s| s.complete_launch(args)).await
            };
            Err(AsyncResponse(Box::new(fut)).into())
        }
    }

    fn wait_for_configuration_done(&self) -> impl Future<Output = Result<(), Error>> {
        let result = self.dap_session.subscribe_requests();
        async move {
            let mut receiver = result?;
            while let Ok((_seq, request)) = receiver.recv().await {
                if let RequestArguments::configurationDone(_) = request {
                    return Ok(());
                }
            }
            bail!("Did not receive configurationDone");
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

        let result = match &self.debuggee_terminal {
            Some(t) => t.attach(|| self.target.launch(&launch_info)),
            None => self.target.launch(&launch_info),
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
        self.process = Initialized(process);
        self.terminate_on_disconnect = true;

        // LLDB sometimes loses the initial stop event.
        if launch_info.launch_flags().intersects(LaunchFlag::StopAtEntry) {
            self.notify_process_stopped();
        }

        if let Some(commands) = args.common.post_run_commands {
            self.exec_commands("postRunCommands", &commands)?;
        }
        self.exit_commands = args.common.exit_commands;
        Ok(ResponseBody::launch)
    }

    fn handle_custom_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        if let Some(commands) = &args.target_create_commands {
            self.exec_commands("targetCreateCommands", &commands)?;
        }
        self.target = Initialized(self.debugger.selected_target());
        self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));
        self.send_event(EventBody::initialized);

        let self_ref = self.self_ref.clone();
        let fut = async move {
            self_ref.map(|s| s.wait_for_configuration_done()).await.await?;
            self_ref.map(|s| s.complete_custom_launch(args)).await
        };
        Err(AsyncResponse(Box::new(fut)).into())
    }

    fn complete_custom_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        if let Some(commands) = args.process_create_commands.as_ref().or(args.common.pre_run_commands.as_ref()) {
            self.exec_commands("processCreateCommands", &commands)?;
        }
        self.process = Initialized(self.target.process());
        self.terminate_on_disconnect = true;

        // This is succeptible to race conditions, but probably the best we can do.
        if self.process.state().is_stopped() {
            self.notify_process_stopped();
        }

        self.exit_commands = args.common.exit_commands;
        Ok(ResponseBody::launch)
    }

    fn handle_attach(&mut self, args: AttachRequestArguments) -> Result<ResponseBody, Error> {
        self.common_init_session(&args.common)?;

        if args.program.is_none() && args.pid.is_none() {
            bail!(as_user_error(r#"Either "program" or "pid" is required for attach."#));
        }

        self.target = match &args.program {
            Some(program) => Initialized(self.create_target_from_program(program)?),
            None => Initialized(self.debugger.create_target("", None, None, false)?),
        };
        self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));
        self.send_event(EventBody::initialized);

        let self_ref = self.self_ref.clone();
        let fut = async move {
            self_ref.map(|s| s.wait_for_configuration_done()).await.await?;
            self_ref.map(|s| s.complete_attach(args)).await
        };
        Err(AsyncResponse(Box::new(fut)).into())
    }

    fn complete_attach(&mut self, args: AttachRequestArguments) -> Result<ResponseBody, Error> {
        if let Some(ref commands) = args.common.pre_run_commands {
            self.exec_commands("preRunCommands", commands)?;
        }

        let attach_info = SBAttachInfo::new();
        if let Some(pid) = args.pid {
            let pid = match pid {
                Pid::Number(n) => n as ProcessID,
                Pid::String(s) => match s.parse() {
                    Ok(n) => n,
                    Err(_) => bail!(as_user_error("Process id must be a positive integer.")),
                },
            };
            attach_info.set_process_id(pid);
        } else if let Some(program) = args.program {
            attach_info.set_executable(&self.find_executable(&program));
        } else {
            unreachable!()
        }
        attach_info.set_wait_for_launch(args.wait_for.unwrap_or(false), false);
        attach_info.set_ignore_existing(false);

        let process = match self.target.attach(&attach_info) {
            Ok(process) => process,
            Err(err) => bail!(as_user_error(err)),
        };
        self.console_message(format!("Attached to process {}", process.process_id()));
        self.process = Initialized(process);
        self.terminate_on_disconnect = false;

        if args.common.stop_on_entry.unwrap_or(false) {
            self.notify_process_stopped();
        } else {
            log_errors!(self.process.resume());
        }

        if let Some(commands) = args.common.post_run_commands {
            self.exec_commands("postRunCommands", &commands)?;
        }
        self.exit_commands = args.common.exit_commands;
        Ok(ResponseBody::attach)
    }

    fn create_target_from_program(&self, program: &str) -> Result<SBTarget, Error> {
        match self.debugger.create_target(program, None, None, false) {
            Ok(target) => Ok(target),
            Err(err) => {
                // TODO: use selected platform instead of cfg!(windows)
                if cfg!(windows) && !program.ends_with(".exe") {
                    let program = format!("{}.exe", program);
                    self.debugger.create_target(&program, None, None, false)
                } else {
                    Err(err)
                }
            }
        }
        .map_err(|e| as_user_error(e).into())
    }

    fn find_executable<'a>(&self, program: &'a str) -> Cow<'a, str> {
        // On Windows, also try program + '.exe'
        // TODO: use selected platform instead of cfg!(windows)
        if cfg!(windows) {
            if !Path::new(program).is_file() {
                let program = format!("{}.exe", program);
                if Path::new(&program).is_file() {
                    return program.into();
                }
            }
        }
        program.into()
    }

    fn create_terminal(&mut self, args: &LaunchRequestArguments) -> impl Future {
        if self.target.platform().name() != "host" {
            return future::ready(()).left_future(); // Can't attach to a terminal when remote-debugging.
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
        let fut = Terminal::create(terminal_kind, title, self.terminal_prompt_clear.clone(), self.dap_session.clone());
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

        if let Some(true) = &args_common.reverse_debugging {
            self.send_event(EventBody::capabilities(CapabilitiesEventBody {
                capabilities: Capabilities {
                    supports_step_back: Some(true),
                    ..Default::default()
                },
            }));
        }

        self.relative_path_base = Initialized(match &args_common.relative_path_base {
            Some(base) => base.into(),
            None => env::current_dir()?,
        });

        if let Some(ref settings) = args_common.adapter_settings {
            self.update_adapter_settings_and_caps(settings);
        }

        self.print_console_mode();

        if let Some(commands) = &args_common.init_commands {
            self.exec_commands("initCommands", &commands)?;
        }

        Ok(())
    }

    fn print_console_mode(&self) {
        let message = match self.console_mode {
            ConsoleMode::Commands => "Console is in 'commands' mode, prefix expressions with '?'.",
            ConsoleMode::Evaluate => "Console is in 'evaluation' mode, prefix commands with '/cmd ' or '`'.",
        };
        self.console_message(message);
    }

    fn exec_commands(&self, script_name: &str, commands: &[String]) -> Result<(), Error> {
        self.console_message(format!("Executing script: {}", script_name));
        let interpreter = self.debugger.command_interpreter();
        let mut result = SBCommandReturnObject::new();
        for command in commands {
            result.clear();
            let ok = interpreter.handle_command(&command, &mut result, false);
            debug!("{} -> {:?}, {:?}", command, ok, result);
            let output = result.output().to_string_lossy().into_owned();
            if !output.is_empty() {
                self.console_message(output);
            }
            if !result.succeeded() {
                let err = result.error().to_string_lossy().into_owned();
                self.console_error(err.clone());
                bail!(as_user_error(err))
            }
        }
        Ok(())
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

    fn handle_configuration_done(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn handle_threads(&mut self) -> Result<ThreadsResponseBody, Error> {
        if !self.process.is_initialized() {
            // VSCode may send `threads` request after a failed launch.
            return Ok(ThreadsResponseBody {
                threads: vec![],
            });
        }
        let mut response = ThreadsResponseBody {
            threads: vec![],
        };
        for thread in self.process.threads() {
            let mut descr = format!("{}: tid={}", thread.index_id(), thread.thread_id());
            if let Some(name) = thread.name() {
                log_errors!(write!(descr, " \"{}\"", name));
            }
            response.threads.push(Thread {
                id: thread.thread_id() as i64,
                name: descr,
            });
        }
        Ok(response)
    }

    fn handle_stack_trace(&mut self, args: StackTraceArguments) -> Result<StackTraceResponseBody, Error> {
        let thread = match self.process.thread_by_id(args.thread_id as ThreadID) {
            Some(thread) => thread,
            None => {
                error!("Received invalid thread id in stack trace request.");
                bail!("Invalid thread id.");
            }
        };

        let start_frame = args.start_frame.unwrap_or(0);
        let levels = args.levels.unwrap_or(std::i64::MAX);

        let mut stack_frames = vec![];
        for i in start_frame..(start_frame + levels) {
            let frame = thread.frame_at_index(i as u32);
            if !frame.is_valid() {
                break;
            }

            let key = format!("[{},{}]", thread.index_id(), i);
            let handle = self.var_refs.create(None, &key, Container::StackFrame(frame.clone()));

            let mut stack_frame: StackFrame = Default::default();
            stack_frame.id = handle.get() as i64;
            let pc_address = frame.pc_address();
            stack_frame.name = if let Some(name) = frame.function_name() {
                name.to_owned()
            } else {
                format!("{:X}", pc_address.file_address())
            };

            if !self.in_disassembly(&frame) {
                if let Some(le) = frame.line_entry() {
                    let fs = le.file_spec();
                    if let Some(local_path) = self.map_filespec_to_local(&fs) {
                        stack_frame.line = le.line() as i64;
                        stack_frame.column = le.column() as i64;
                        stack_frame.source = Some(Source {
                            name: Some(local_path.file_name().unwrap().to_string_lossy().into_owned()),
                            path: Some(local_path.to_string_lossy().into_owned()),
                            ..Default::default()
                        });
                    }
                }
            } else {
                let pc_addr = frame.pc();
                let dasm = self.disassembly.from_address(pc_addr)?;
                stack_frame.line = dasm.line_num_by_address(pc_addr) as i64;
                stack_frame.column = 0;
                stack_frame.source = Some(Source {
                    name: Some(dasm.source_name().to_owned()),
                    source_reference: Some(handles::to_i64(Some(dasm.handle()))),
                    ..Default::default()
                });
                stack_frame.presentation_hint = Some("subtle".to_owned());
            }
            stack_frames.push(stack_frame);
        }

        Ok(StackTraceResponseBody {
            stack_frames: stack_frames,
            total_frames: Some(thread.num_frames() as i64),
        })
    }

    fn in_disassembly(&mut self, frame: &SBFrame) -> bool {
        match self.show_disassembly {
            ShowDisassembly::Always => true,
            ShowDisassembly::Never => false,
            ShowDisassembly::Auto => {
                if let Some(le) = frame.line_entry() {
                    self.map_filespec_to_local(&le.file_spec()).is_none()
                } else {
                    true
                }
            }
        }
    }

    fn handle_pause(&mut self, _args: PauseArguments) -> Result<(), Error> {
        match self.process.stop() {
            Ok(()) => Ok(()),
            Err(error) => {
                if self.process.state().is_stopped() {
                    // Did we lose a 'stopped' event?
                    self.notify_process_stopped();
                    Ok(())
                } else {
                    bail!(as_user_error(error));
                }
            }
        }
    }

    fn handle_continue(&mut self, _args: ContinueArguments) -> Result<ContinueResponseBody, Error> {
        self.before_resume();
        match self.process.resume() {
            Ok(()) => Ok(ContinueResponseBody {
                all_threads_continued: Some(true),
            }),
            Err(err) => {
                if self.process.state().is_running() {
                    // Did we lose a 'running' event?
                    self.notify_process_running();
                    Ok(ContinueResponseBody {
                        all_threads_continued: Some(true),
                    })
                } else {
                    bail!(as_user_error(err))
                }
            }
        }
    }

    fn handle_next(&mut self, args: NextArguments) -> Result<(), Error> {
        let thread = match self.process.thread_by_id(args.thread_id as ThreadID) {
            Some(thread) => thread,
            None => {
                error!("Received invalid thread id in step request.");
                bail!("Invalid thread id.");
            }
        };

        self.before_resume();
        let frame = thread.frame_at_index(0);
        if !self.in_disassembly(&frame) {
            thread.step_over(RunMode::OnlyDuringStepping);
        } else {
            thread.step_instruction(true);
        }
        Ok(())
    }

    fn handle_step_in(&mut self, args: StepInArguments) -> Result<(), Error> {
        let thread = match self.process.thread_by_id(args.thread_id as ThreadID) {
            Some(thread) => thread,
            None => {
                error!("Received invalid thread id in step-in request.");
                bail!("Invalid thread id.")
            }
        };

        self.before_resume();
        let frame = thread.frame_at_index(0);
        if !self.in_disassembly(&frame) {
            thread.step_into(RunMode::OnlyDuringStepping);
        } else {
            thread.step_instruction(false);
        }
        Ok(())
    }

    fn handle_step_out(&mut self, args: StepOutArguments) -> Result<(), Error> {
        self.before_resume();
        let thread = self.process.thread_by_id(args.thread_id as ThreadID).ok_or("thread_id")?;
        thread.step_out();
        if self.process.state().is_stopped() {
            self.notify_process_stopped();
        }
        Ok(())
    }

    fn handle_step_back(&mut self, args: StepBackArguments) -> Result<(), Error> {
        self.before_resume();
        self.show_disassembly = ShowDisassembly::Always; // Reverse line-step is not supported, so we switch to disassembly mode.
        self.reverse_exec(&[
            &format!("process plugin packet send Hc{:x}", args.thread_id), // select thread
            "process plugin packet send bs",                               // reverse-step
            "process plugin packet send bs",                               // reverse-step so we can forward step
            "stepi", // forward-step to refresh LLDB's cached debuggee state
        ])
    }

    fn handle_reverse_continue(&mut self, args: ReverseContinueArguments) -> Result<(), Error> {
        self.before_resume();
        self.reverse_exec(&[
            &format!("process plugin packet send Hc{:x}", args.thread_id), // select thread
            "process plugin packet send bc",                               // reverse-continue
            "process plugin packet send bs",                               // reverse-step so we can forward step
            "stepi", // forward-step to refresh LLDB's cached debuggee state
        ])
    }

    fn reverse_exec(&mut self, commands: &[&str]) -> Result<(), Error> {
        let interp = self.debugger.command_interpreter();
        let mut result = SBCommandReturnObject::new();
        for command in commands {
            interp.handle_command(&command, &mut result, false);
            if !result.succeeded() {
                let error = into_string_lossy(result.error());
                self.console_error(error.clone());
                bail!(error);
            }
        }
        Ok(())
    }

    fn handle_source(&mut self, args: SourceArguments) -> Result<SourceResponseBody, Error> {
        let handle = handles::from_i64(args.source_reference)?;
        let dasm = self.disassembly.find_by_handle(handle).unwrap();
        Ok(SourceResponseBody {
            content: dasm.get_source_text(),
            mime_type: Some("text/x-lldb.disassembly".to_owned()),
        })
    }

    fn handle_completions(&mut self, args: CompletionsArguments) -> Result<CompletionsResponseBody, Error> {
        if !self.command_completions {
            bail!("Completions are disabled");
        }
        let (text, cursor_column) = match self.console_mode {
            ConsoleMode::Commands => (&args.text[..], args.column - 1),
            ConsoleMode::Evaluate => {
                if args.text.starts_with('`') {
                    (&args.text[1..], args.column - 2)
                } else if args.text.starts_with("/cmd ") {
                    (&args.text[5..], args.column - 6)
                } else {
                    // TODO: expression completions
                    return Ok(CompletionsResponseBody {
                        targets: vec![],
                    });
                }
            }
        };

        // Work around LLDB crash when text starts with non-alphabetic character.
        if let Some(c) = text.chars().next() {
            if !c.is_alphabetic() {
                return Ok(CompletionsResponseBody {
                    targets: vec![],
                });
            }
        }

        // Compute cursor position inside text in as byte offset.
        let cursor_index = text.char_indices().skip(cursor_column as usize).next().map(|p| p.0).unwrap_or(text.len());

        let interpreter = self.debugger.command_interpreter();
        let targets = match interpreter.handle_completions(text, cursor_index as u32, None) {
            None => vec![],
            Some((common_continuation, completions)) => {
                // LLDB completions usually include some prefix of the string being completed, without telling us what that prefix is.
                // For example, completing "set show tar" might return ["target.arg0", "target.auto-apply-fixits", ...].

                // Take a slice up to the cursor, split it on whitespaces, then get the last part.
                // This is the (likely) prefix of completions returned by LLDB.
                let prefix = &text[..cursor_index].split_whitespace().next_back().unwrap_or_default();
                let prefix_len = prefix.chars().count();
                let extended_prefix = format!("{}{}", prefix, common_continuation);

                let mut targets = vec![];
                for completion in completions {
                    // Check if we guessed prefix correctly
                    let item = if completion.starts_with(&extended_prefix) {
                        CompletionItem {
                            label: completion,
                            start: Some(args.column - prefix_len as i64),
                            length: Some(prefix_len as i64),
                            ..Default::default()
                        }
                    } else {
                        // Let VSCode apply its own heuristics to figure out the prefix.
                        CompletionItem {
                            label: completion,
                            ..Default::default()
                        }
                    };
                    targets.push(item);
                }
                targets
            }
        };

        Ok(CompletionsResponseBody {
            targets,
        })
    }

    fn handle_goto_targets(&mut self, args: GotoTargetsArguments) -> Result<GotoTargetsResponseBody, Error> {
        let targets = vec![GotoTarget {
            id: 1,
            label: format!("line {}", args.line),
            line: args.line,
            end_line: None,
            column: None,
            end_column: None,
            instruction_pointer_reference: None,
        }];
        self.last_goto_request = Some(args);
        Ok(GotoTargetsResponseBody {
            targets,
        })
    }

    fn handle_goto(&mut self, args: GotoArguments) -> Result<(), Error> {
        match &self.last_goto_request {
            None => bail!("Unexpected goto message."),
            Some(ref goto_args) => {
                let thread_id = args.thread_id as u64;
                match self.process.thread_by_id(thread_id) {
                    None => bail!("Invalid thread id"),
                    Some(thread) => match goto_args.source.source_reference {
                        // Disassembly
                        Some(source_ref) => {
                            let handle = handles::from_i64(source_ref)?;
                            let dasm = self.disassembly.find_by_handle(handle).ok_or("source_ref")?;
                            let addr = dasm.address_by_line_num(goto_args.line as u32);
                            let frame = thread.frame_at_index(0).check().ok_or("frame 0")?;
                            if frame.set_pc(addr) {
                                self.refresh_client_display(Some(thread_id));
                                Ok(())
                            } else {
                                bail!(as_user_error("Failed to set the instruction pointer."));
                            }
                        }
                        // Normal source file
                        None => {
                            let filespec = SBFileSpec::from(goto_args.source.path.as_ref().ok_or("source.path")?);
                            match thread.jump_to_line(&filespec, goto_args.line as u32) {
                                Ok(()) => {
                                    self.last_goto_request = None;
                                    self.refresh_client_display(Some(thread_id));
                                    Ok(())
                                }
                                Err(err) => {
                                    bail!(as_user_error(err))
                                }
                            }
                        }
                    },
                }
            }
        }
    }

    fn handle_restart_frame(&mut self, args: RestartFrameArguments) -> Result<(), Error> {
        let handle = handles::from_i64(args.frame_id)?;
        let frame = match self.var_refs.get(handle) {
            Some(Container::StackFrame(ref f)) => f.clone(),
            _ => bail!("Invalid frameId"),
        };
        let thread = frame.thread();
        thread.return_from_frame(&frame)?;
        self.send_event(EventBody::stopped(StoppedEventBody {
            thread_id: Some(thread.thread_id() as i64),
            all_threads_stopped: Some(true),
            reason: "restart".into(),
            ..Default::default()
        }));
        Ok(())
    }

    fn handle_data_breakpoint_info(
        &mut self,
        args: DataBreakpointInfoArguments,
    ) -> Result<DataBreakpointInfoResponseBody, Error> {
        let container_handle = handles::from_i64(args.variables_reference.ok_or("variables_reference")?)?;
        let container = self.var_refs.get(container_handle).expect("Invalid variables reference");
        let child = match container {
            Container::SBValue(container) => container.child_member_with_name(&args.name),
            Container::Locals(frame) => frame.find_variable(&args.name),
            Container::Globals(frame) => frame.find_value(&args.name, ValueType::VariableGlobal),
            Container::Statics(frame) => frame.find_value(&args.name, ValueType::VariableStatic),
            _ => None,
        };
        if let Some(child) = child {
            let addr = child.load_address();
            if addr != lldb::INVALID_ADDRESS {
                let size = child.byte_size();
                if self.is_valid_watchpoint_size(size) {
                    let data_id = format!("{}/{}", addr, size);
                    let desc = child.name().unwrap_or("");
                    Ok(DataBreakpointInfoResponseBody {
                        data_id: Some(data_id),
                        access_types: Some(vec![
                            DataBreakpointAccessType::Read,
                            DataBreakpointAccessType::Write,
                            DataBreakpointAccessType::ReadWrite,
                        ]),
                        description: format!("{} bytes at {:X} ({})", size, addr, desc),
                        ..Default::default()
                    })
                } else {
                    Ok(DataBreakpointInfoResponseBody {
                        data_id: None,
                        description: "Invalid watchpoint size.".into(),
                        ..Default::default()
                    })
                }
            } else {
                Ok(DataBreakpointInfoResponseBody {
                    data_id: None,
                    description: "This variable doesn't have an address.".into(),
                    ..Default::default()
                })
            }
        } else {
            Ok(DataBreakpointInfoResponseBody {
                data_id: None,
                description: "Variable not found.".into(),
                ..Default::default()
            })
        }
    }

    fn is_valid_watchpoint_size(&self, size: usize) -> bool {
        let addr_size = self.target.address_byte_size();
        match addr_size {
            4 => match size {
                1 | 2 | 4 => true,
                _ => false,
            },
            8 => match size {
                1 | 2 | 4 | 8 => true,
                _ => false,
            },
            _ => true, // No harm in allowing to set an invalid watchpoint, other than user confusion.
        }
    }

    fn handle_set_data_breakpoints(
        &mut self,
        args: SetDataBreakpointsArguments,
    ) -> Result<SetDataBreakpointsResponseBody, Error> {
        self.target.delete_all_watchpoints();
        let mut watchpoints = vec![];
        for wp in args.breakpoints {
            let mut parts = wp.data_id.split('/');
            let addr = parts.next().ok_or("")?.parse::<u64>()?;
            let size = parts.next().ok_or("")?.parse::<usize>()?;
            let (read, write) = match wp.access_type {
                None => (false, true),
                Some(DataBreakpointAccessType::Read) => (true, false),
                Some(DataBreakpointAccessType::Write) => (false, true),
                Some(DataBreakpointAccessType::ReadWrite) => (true, true),
            };
            let when = match (read, write) {
                (true, false) => "read",
                (false, true) => "write",
                (true, true) => "read and write",
                _ => unreachable!(),
            };
            let res = match self.target.watch_address(addr, size, read, write) {
                Ok(_wp) => Breakpoint {
                    verified: true,
                    message: Some(format!("Break on {}", when)),
                    ..Default::default()
                },
                Err(err) => Breakpoint {
                    verified: false,
                    message: Some(err.to_string()),
                    ..Default::default()
                },
            };
            watchpoints.push(res);
        }
        Ok(SetDataBreakpointsResponseBody {
            breakpoints: watchpoints,
        })
    }

    fn handle_disconnect(&mut self, args: Option<DisconnectArguments>) -> Result<(), Error> {
        if let Some(commands) = &self.exit_commands {
            self.exec_commands("exitCommands", &commands)?;
        }

        // Let go of the terminal helper connection
        self.debuggee_terminal = None;

        if let Initialized(ref process) = self.process {
            let state = process.state();
            if state.is_alive() {
                let terminate = match args {
                    Some(args) => match args.terminate_debuggee {
                        Some(terminate) => terminate,
                        None => self.terminate_on_disconnect,
                    },
                    None => self.terminate_on_disconnect,
                };
                if terminate {
                    process.kill()?;
                } else {
                    process.detach()?;
                }
            }
        }

        Ok(())
    }

    fn handle_read_memory(&mut self, args: ReadMemoryArguments) -> Result<ReadMemoryResponseBody, Error> {
        let mem_ref = parse_int::parse::<i64>(&args.memory_reference)?;
        let offset = args.offset.unwrap_or(0);
        let count = args.count as usize;
        let address = (mem_ref + offset) as lldb::Address;
        if let Ok(region_info) = self.process.get_memory_region_info(address) {
            if region_info.is_readable() {
                let to_read = cmp::min(count, (region_info.region_end() - address) as usize);
                let mut buffer = Vec::new();
                buffer.resize(to_read, 0);
                if let Ok(bytes_read) = self.process.read_memory(address, buffer.as_mut_slice()) {
                    buffer.resize(bytes_read, 0);
                    return Ok(ReadMemoryResponseBody {
                        address: format!("0x{:X}", address),
                        unreadable_bytes: Some((count - bytes_read) as i64),
                        data: Some(base64::encode(buffer)),
                    });
                }
            }
        }
        Ok(ReadMemoryResponseBody {
            address: format!("0x{:X}", address),
            unreadable_bytes: Some(args.count),
            data: None,
        })
    }

    fn handle_write_memory(&mut self, args: WriteMemoryArguments) -> Result<WriteMemoryResponseBody, Error> {
        let mem_ref = parse_int::parse::<i64>(&args.memory_reference)?;
        let offset = args.offset.unwrap_or(0);
        let address = (mem_ref + offset) as lldb::Address;
        let data = base64::decode(&args.data)?;
        let allow_partial = args.allow_partial.unwrap_or(false);
        if let Ok(region_info) = self.process.get_memory_region_info(address) {
            if region_info.is_writable() {
                let to_write = cmp::min(data.len(), (region_info.region_end() - address) as usize);
                if allow_partial || to_write == data.len() {
                    if let Ok(bytes_written) = self.process.write_memory(address, &data) {
                        return Ok(WriteMemoryResponseBody {
                            bytes_written: Some(bytes_written as i64),
                            ..Default::default()
                        });
                    }
                }
            }
        }
        if allow_partial {
            Ok(WriteMemoryResponseBody {
                bytes_written: Some(0),
                ..Default::default()
            })
        } else {
            Err(as_user_error(format!("Cannot write {} bytes at {:08X}", data.len(), address)).into())
        }
    }

    fn handle_symbols(&mut self, args: SymbolsRequest) -> Result<SymbolsResponse, Error> {
        use fuzzy_matcher::FuzzyMatcher;
        let matcher = fuzzy_matcher::clangd::ClangdMatcher::default().ignore_case();
        let mut symbols = vec![];
        'outer: for imodule in 0..self.target.num_modules() {
            let module = self.target.module_at_index(imodule);
            for isymbol in 0..module.num_symbols() {
                let symbol = module.symbol_at_index(isymbol);
                let ty = symbol.type_();
                match ty {
                    SymbolType::Code | SymbolType::Data => {
                        let name = symbol.display_name();
                        if let Some(_) = matcher.fuzzy_match(name, &args.filter) {
                            let start_addr = symbol.start_address().load_address(&self.target);

                            let location = if let Some(le) = symbol.start_address().line_entry() {
                                let fs = le.file_spec();
                                if let Some(local_path) = self.map_filespec_to_local(&fs) {
                                    let source = Source {
                                        name: Some(local_path.file_name().unwrap().to_string_lossy().into_owned()),
                                        path: Some(local_path.to_string_lossy().into_owned()),
                                        ..Default::default()
                                    };
                                    Some((source, le.line()))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            let symbol = Symbol {
                                name: name.into(),
                                type_: format!("{:?}", ty),
                                address: format!("0x{:X}", start_addr),
                                location: location,
                            };
                            symbols.push(symbol);
                        }
                    }
                    _ => {}
                }

                if symbols.len() >= args.max_results as usize {
                    break 'outer;
                }
            }
        }
        Ok(SymbolsResponse {
            symbols,
        })
    }

    fn handle_adapter_settings(&mut self, args: AdapterSettings) -> Result<(), Error> {
        let old_console_mode = self.console_mode;
        self.update_adapter_settings_and_caps(&args);
        if self.console_mode != old_console_mode {
            self.print_console_mode();
        }
        if self.process.state().is_stopped() {
            self.refresh_client_display(None);
        }
        Ok(())
    }

    fn update_adapter_settings_and_caps(&mut self, settings: &AdapterSettings) {
        self.update_adapter_settings(&settings);
        if settings.evaluate_for_hovers.is_some()
            || settings.command_completions.is_some()
            || settings.source_languages.is_some()
        {
            self.send_event(EventBody::capabilities(CapabilitiesEventBody {
                capabilities: self.make_capabilities(),
            }));
        }
    }

    fn update_adapter_settings(&mut self, settings: &AdapterSettings) {
        self.global_format = match settings.display_format {
            None => self.global_format,
            Some(DisplayFormat::Auto) => Format::Default,
            Some(DisplayFormat::Decimal) => Format::Decimal,
            Some(DisplayFormat::Hex) => Format::Hex,
            Some(DisplayFormat::Binary) => Format::Binary,
        };
        self.show_disassembly = settings.show_disassembly.unwrap_or(self.show_disassembly);
        self.deref_pointers = settings.dereference_pointers.unwrap_or(self.deref_pointers);
        self.suppress_missing_files = settings.suppress_missing_source_files.unwrap_or(self.suppress_missing_files);
        self.evaluate_for_hovers = settings.evaluate_for_hovers.unwrap_or(self.evaluate_for_hovers);
        self.command_completions = settings.command_completions.unwrap_or(self.command_completions);
        if let Some(timeout) = settings.evaluation_timeout {
            self.evaluation_timeout = time::Duration::from_millis((timeout * 1000.0) as u64);
        }
        if let Some(ref terminal_prompt_clear) = settings.terminal_prompt_clear {
            self.terminal_prompt_clear = Some(terminal_prompt_clear.clone());
        }
        if let Some(ref source_languages) = settings.source_languages {
            self.source_languages = source_languages.clone();
        }
        if let Some(console_mode) = settings.console_mode {
            self.console_mode = console_mode;
        }
    }

    // Send fake stop event to force VSCode to refresh its UI state.
    fn refresh_client_display(&mut self, thread_id: Option<ThreadID>) {
        let thread_id = match thread_id {
            Some(tid) => tid,
            None => self.process.selected_thread().thread_id(),
        };
        self.send_event(EventBody::stopped(StoppedEventBody {
            thread_id: Some(thread_id as i64),
            all_threads_stopped: Some(true),
            ..Default::default()
        }));
    }

    fn before_resume(&mut self) {
        self.var_refs.reset();
        self.selected_frame_changed = false;
    }

    fn handle_debug_event(&mut self, event: SBEvent) {
        debug!("Debug event: {:?}", event);
        if let Some(process_event) = event.as_process_event() {
            self.handle_process_event(&process_event);
        } else if let Some(target_event) = event.as_target_event() {
            self.handle_target_event(&target_event);
        } else if let Some(bp_event) = event.as_breakpoint_event() {
            self.handle_breakpoint_event(&bp_event);
        } else if let Some(thread_event) = event.as_thread_event() {
            self.handle_thread_event(&thread_event);
        }
    }

    fn handle_process_event(&mut self, process_event: &SBProcessEvent) {
        let flags = process_event.as_event().flags();
        if flags & SBProcessEvent::BroadcastBitStateChanged != 0 {
            match process_event.process_state() {
                ProcessState::Running | ProcessState::Stepping => self.notify_process_running(),
                ProcessState::Stopped => {
                    if !process_event.restarted() {
                        self.notify_process_stopped()
                    }
                }
                ProcessState::Crashed | ProcessState::Suspended => self.notify_process_stopped(),
                ProcessState::Exited => {
                    let exit_code = self.process.exit_status() as i64;
                    self.console_message(format!("Process exited with code {}.", exit_code));
                    self.send_event(EventBody::exited(ExitedEventBody {
                        exit_code,
                    }));
                    self.send_event(EventBody::terminated(TerminatedEventBody {
                        restart: None,
                    }));
                }
                ProcessState::Detached => {
                    self.console_message("Detached from debuggee.");
                    self.send_event(EventBody::terminated(TerminatedEventBody {
                        restart: None,
                    }));
                }
                _ => (),
            }
        }
        if flags & (SBProcessEvent::BroadcastBitSTDOUT | SBProcessEvent::BroadcastBitSTDERR) != 0 {
            let read_stdout = |b: &mut [u8]| self.process.read_stdout(b);
            let read_stderr = |b: &mut [u8]| self.process.read_stderr(b);
            let (read_stream, category): (&dyn for<'r> Fn(&mut [u8]) -> usize, &str) =
                if flags & SBProcessEvent::BroadcastBitSTDOUT != 0 {
                    (&read_stdout, "stdout")
                } else {
                    (&read_stderr, "stderr")
                };
            let mut buffer = [0; 1024];
            let mut read = read_stream(&mut buffer);
            while read > 0 {
                self.send_event(EventBody::output(OutputEventBody {
                    category: Some(category.to_owned()),
                    output: String::from_utf8_lossy(&buffer[..read]).into_owned(),
                    ..Default::default()
                }));
                read = read_stream(&mut buffer);
            }
        }
    }

    fn notify_process_running(&mut self) {
        self.send_event(EventBody::continued(ContinuedEventBody {
            all_threads_continued: Some(true),
            thread_id: 0,
        }));
    }

    fn notify_process_stopped(&mut self) {
        // Find thread that has caused this stop
        let mut stopped_thread;
        // Check the currently selected thread first
        let selected_thread = self.process.selected_thread();
        stopped_thread = match selected_thread.stop_reason() {
            StopReason::Invalid | //.
            StopReason::None => None,
            _ => Some(selected_thread),
        };
        // Fall back to scanning all threads in the process
        if stopped_thread.is_none() {
            for thread in self.process.threads() {
                match thread.stop_reason() {
                    StopReason::Invalid | //.
                    StopReason::None => (),
                    _ => {
                        self.process.set_selected_thread(&thread);
                        stopped_thread = Some(thread);
                        break;
                    }
                }
            }
        }
        // Analyze stop reason
        let (stop_reason_str, description) = match stopped_thread {
            Some(ref stopped_thread) => {
                let stop_reason = stopped_thread.stop_reason();
                match stop_reason {
                    StopReason::Breakpoint => ("breakpoint", None),
                    StopReason::Trace | //.
                    StopReason::PlanComplete => ("step", None),
                    StopReason::Watchpoint => ("watchpoint", None),
                    StopReason::Signal => ("signal", Some(stopped_thread.stop_description())),
                    StopReason::Exception => ("exception", Some(stopped_thread.stop_description())),
                    _ => ("unknown", Some(stopped_thread.stop_description())),
                }
            }
            None => ("unknown", None),
        };

        if let Some(description) = &description {
            self.console_error(format!("Stop reason: {}", description));
        }

        self.send_event(EventBody::stopped(StoppedEventBody {
            all_threads_stopped: Some(true),
            thread_id: stopped_thread.map(|t| t.thread_id() as i64),
            reason: stop_reason_str.to_owned(),
            description: None,
            text: description,
            preserve_focus_hint: None,
            ..Default::default()
        }));

        if let Some(python) = &self.python {
            python.modules_loaded(&mut self.loaded_modules.iter());
        }
        self.loaded_modules.clear();
    }

    fn handle_target_event(&mut self, event: &SBTargetEvent) {
        let flags = event.as_event().flags();
        if flags & SBTargetEvent::BroadcastBitModulesLoaded != 0 {
            for module in event.modules() {
                self.send_event(EventBody::module(ModuleEventBody {
                    reason: "new".to_owned(),
                    module: self.make_module_detail(&module),
                }));

                // Running scripts during target execution seems to trigger a bug in LLDB,
                // so we defer loaded module notification till next stop.
                self.loaded_modules.push(module);
            }
        } else if flags & SBTargetEvent::BroadcastBitSymbolsLoaded != 0 {
            for module in event.modules() {
                self.send_event(EventBody::module(ModuleEventBody {
                    reason: "changed".to_owned(),
                    module: self.make_module_detail(&module),
                }));
            }
        } else if flags & SBTargetEvent::BroadcastBitModulesUnloaded != 0 {
            for module in event.modules() {
                self.send_event(EventBody::module(ModuleEventBody {
                    reason: "removed".to_owned(),
                    module: Module {
                        id: serde_json::Value::String(self.module_id(&module)),
                        ..Default::default()
                    },
                }));
            }
        }
    }

    fn module_id(&self, module: &SBModule) -> String {
        let header_addr = module.object_header_address();
        if header_addr.is_valid() {
            format!("{:X}", header_addr.load_address(&self.target))
        } else {
            // header_addr not available on Windows, fall back to path
            module.file_spec().path().display().to_string()
        }
    }

    fn make_module_detail(&self, module: &SBModule) -> Module {
        let mut msg = Module {
            id: serde_json::Value::String(self.module_id(&module)),
            name: module.file_spec().filename().display().to_string(),
            path: Some(module.file_spec().path().display().to_string()),
            ..Default::default()
        };

        let header_addr = module.object_header_address();
        if header_addr.is_valid() {
            msg.address_range = Some(format!("{:X}", header_addr.load_address(&self.target)));
        }

        let symbols = module.symbol_file_spec();
        if symbols.is_valid() {
            msg.symbol_status = Some("Symbols loaded.".into());
            msg.symbol_file_path = Some(module.symbol_file_spec().path().display().to_string());
        } else {
            msg.symbol_status = Some("Symbols not found".into())
        }

        msg
    }

    fn handle_thread_event(&mut self, event: &SBThreadEvent) {
        let flags = event.as_event().flags();
        if flags & SBThreadEvent::BroadcastBitSelectedFrameChanged != 0 {
            self.selected_frame_changed = true;
        }
    }

    // Maps remote file path to local file path.
    // The bulk of this work is done by LLDB itself (via target.source-map), in addition to which:
    // - if `filespec` contains a relative path, we convert it to an absolute one using relative_path_base
    //   (which is normally initialized to ${workspaceFolder}) as a base.
    // - we check whether the local file actually exists, and suppress it (if `suppress_missing_files` is true),
    //   to prevent VSCode from prompting to create them.
    fn map_filespec_to_local(&self, filespec: &SBFileSpec) -> Option<Rc<PathBuf>> {
        if !filespec.is_valid() {
            return None;
        } else {
            let source_path = filespec.path();
            let mut source_map_cache = self.source_map_cache.borrow_mut();
            match source_map_cache.get(&source_path) {
                Some(mapped_path) => mapped_path.clone(),
                None => {
                    let mut path = filespec.path();
                    // Make sure the path is absolute.
                    if path.is_relative() {
                        path = self.relative_path_base.join(path);
                    }
                    path = normalize_path(path);
                    // VSCode sometimes fails to compare equal paths that differ in casing.
                    let mapped_path = match get_fs_path_case(&path) {
                        Ok(path) if path.is_file() => Some(Rc::new(path)),
                        _ => {
                            if self.suppress_missing_files {
                                None
                            } else {
                                Some(Rc::new(path))
                            }
                        }
                    };
                    // Cache the result, so we don't have to probe file system again for the same path.
                    source_map_cache.insert(source_path, mapped_path.clone());
                    mapped_path
                }
            }
        }
    }

    fn context_from_frame(&self, frame: Option<&SBFrame>) -> SBExecutionContext {
        match frame {
            Some(frame) => SBExecutionContext::from_frame(&frame),
            None => match self.process {
                Initialized(ref process) => {
                    let thread = process.selected_thread();
                    SBExecutionContext::from_thread(&thread)
                }
                NotInitialized => {
                    let target = self.debugger.selected_target();
                    SBExecutionContext::from_target(&target)
                }
            },
        }
    }
}

impl Drop for DebugSession {
    fn drop(&mut self) {
        debug!("DebugSession::drop()");
    }
}

fn into_string_lossy(cstr: &CStr) -> String {
    cstr.to_string_lossy().into_owned()
}
