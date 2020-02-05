use std;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::CStr;
use std::fmt::Write;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str;
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time;

use futures;
use futures::prelude::*;
use log::{debug, error, info};
use serde_derive::*;
use serde_json;

use crate::cancellation::{CancellationSource, CancellationToken};
use crate::debug_protocol::*;
use crate::disassembly;
use crate::error::Error;
use crate::expressions::{self, FormatSpec, HitCondition, PreparedExpression};
use crate::fsutil::{is_same_path, normalize_path};
use crate::handles::{self, Handle, HandleTree};
use crate::must_initialize::{Initialized, MustInitialize, NotInitialized};
use crate::python;
use crate::terminal::Terminal;

use python::PythonInterface;

use lldb::*;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdapterParameters {
    source_languages: Option<Vec<String>>,
}

type AsyncResponder = dyn FnOnce(&mut DebugSession) -> Result<ResponseBody, Error>;

#[derive(Debug, Clone)]
enum BreakpointKind {
    Location,
    Address,
    Function,
    Exception,
}

#[derive(Debug, Clone)]
struct BreakpointInfo {
    id: BreakpointID,
    breakpoint: SBBreakpoint,
    kind: BreakpointKind,
    condition: Option<String>,
    log_message: Option<String>,
    hit_condition: Option<HitCondition>,
    hit_count: u32,
}

enum Container {
    StackFrame(SBFrame),
    Locals(SBFrame),
    Statics(SBFrame),
    Globals(SBFrame),
    Registers(SBFrame),
    SBValue(SBValue),
}

struct BreakpointsState {
    source: HashMap<PathBuf, HashMap<i64, BreakpointID>>,
    assembly: HashMap<Handle, HashMap<i64, BreakpointID>>,
    function: HashMap<String, BreakpointID>,
    breakpoint_infos: HashMap<BreakpointID, BreakpointInfo>,
}

enum InputEvent {
    ProtocolMessage(ProtocolMessage),
    DebugEvent(SBEvent),
    Invoke(Box<dyn FnOnce() + Send>),
}

pub struct DebugSession {
    send_message: RefCell<futures::sync::mpsc::Sender<ProtocolMessage>>,
    message_seq: Cell<u32>,
    incoming_send: std::sync::mpsc::SyncSender<InputEvent>,
    shutdown: CancellationSource,
    event_listener: SBListener,
    self_ref: MustInitialize<Weak<Mutex<DebugSession>>>,
    debugger: SBDebugger,
    target: MustInitialize<SBTarget>,
    process: MustInitialize<SBProcess>,
    process_was_launched: bool,
    on_configuration_done: Option<(u32, Box<AsyncResponder>)>,
    python: Option<Box<PythonInterface>>,
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
    default_expr_type: Expressions,

    global_format: Format,
    show_disassembly: ShowDisassembly,
    deref_pointers: bool,
    console_mode: ConsoleMode,
    suppress_missing_files: bool,
    evaluation_timeout: time::Duration,
    source_languages: Vec<String>,
    default_panic_breakpoint: bool,
    default_catch_breakpoint: bool
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////

unsafe impl Send for DebugSession {}

impl DebugSession {
    pub fn new(
        settings: AdapterSettings,
    ) -> impl Stream<Item = ProtocolMessage, Error = ()> + Sink<SinkItem = ProtocolMessage, SinkError = ()> {
        let (incoming_send, incoming_recv) = std::sync::mpsc::sync_channel::<InputEvent>(100);
        let (outgoing_send, outgoing_recv) = futures::sync::mpsc::channel::<ProtocolMessage>(100);

        let shutdown = CancellationSource::new();
        let shutdown_token = shutdown.cancellation_token();
        let event_listener = SBListener::new_with_name("DebugSession");

        {
            let shutdown_token = shutdown_token.clone();
            let event_listener = event_listener.clone();
            let sender = incoming_send.clone();

            thread::Builder::new().name("Event listener".into()).spawn(move || {
                let mut event = SBEvent::new();
                while !shutdown_token.is_cancelled() {
                    if event_listener.wait_for_event(1, &mut event) {
                        match sender.try_send(InputEvent::DebugEvent(event)) {
                            Err(err) => error!("Could not send event to DebugSession: {:?}", err),
                            Ok(_) => {}
                        }
                        event = SBEvent::new();
                    }
                }
                debug!("### Shutting down event listener thread");
            });
        }

        let send_message = RefCell::new(outgoing_send);

        let debugger = SBDebugger::create(false);
        debugger.set_async_mode(true);

        struct PythonEventSink(RefCell<futures::sync::mpsc::Sender<ProtocolMessage>>);
        impl python::EventSink for PythonEventSink {
            fn display_html(&self, html: String, title: Option<String>, position: Option<i32>, reveal: bool) {
                let event = ProtocolMessage::Event(Event {
                    seq: 0,
                    body: EventBody::displayHtml(DisplayHtmlEventBody {
                        html,
                        title,
                        position,
                        reveal,
                    }),
                });
                self.0.borrow_mut().try_send(event);
            }
        }
        let event_sink = Box::new(PythonEventSink(send_message.clone()));
        let python = match python::PythonInterface::new(debugger.command_interpreter(), event_sink) {
            Ok(python) => Some(python),
            Err(err) => {
                error!("Initialize Python interpreter: {}", err);
                None
            }
        };

        let mut debug_session = DebugSession {
            send_message: send_message,
            incoming_send: incoming_send.clone(),
            message_seq: Cell::new(1),
            shutdown: shutdown,
            self_ref: NotInitialized,
            debugger: debugger,
            target: NotInitialized,
            process: NotInitialized,
            process_was_launched: false,
            event_listener: event_listener,
            on_configuration_done: None,
            python: python,
            breakpoints: RefCell::new(BreakpointsState {
                source: HashMap::new(),
                assembly: HashMap::new(),
                function: HashMap::new(),
                breakpoint_infos: HashMap::new(),
            }),
            var_refs: HandleTree::new(),
            disassembly: NotInitialized,
            source_map_cache: RefCell::new(HashMap::new()),
            loaded_modules: Vec::new(),
            relative_path_base: NotInitialized,
            exit_commands: None,
            debuggee_terminal: None,
            selected_frame_changed: false,
            last_goto_request: None,
            default_expr_type: Expressions::Simple,

            global_format: Format::Default,
            show_disassembly: ShowDisassembly::Auto,
            deref_pointers: true,
            console_mode: ConsoleMode::Commands,
            source_languages: vec!["cpp".into()],
            suppress_missing_files: true,
            evaluation_timeout: time::Duration::from_secs(5),
            default_panic_breakpoint: true,
            default_catch_breakpoint: false
        };
        debug_session.update_adapter_settings(&settings);

        let debug_session = Arc::new(Mutex::new(debug_session));
        let weak = Arc::downgrade(&debug_session);
        debug_session.lock().unwrap().self_ref = MustInitialize::Initialized(weak);

        thread::Builder::new().name("DebugSession".into()).spawn(move || loop {
            match incoming_recv.recv() {
                Ok(event) => match event {
                    InputEvent::ProtocolMessage(msg) => debug_session.lock().unwrap().handle_message(msg),
                    InputEvent::DebugEvent(event) => debug_session.lock().unwrap().handle_debug_event(event),
                    InputEvent::Invoke(func) => func(),
                },
                Err(_) => break,
            }
        });

        AsyncDebugSession {
            incoming_send,
            outgoing_recv,
            shutdown_token,
        }
    }

    fn handle_message(&mut self, message: ProtocolMessage) {
        match message {
            ProtocolMessage::Request(request) => self.handle_request(request),
            ProtocolMessage::Response(response) => self.handle_response(response),
            ProtocolMessage::Event(event) => error!("No handler for event message: {:?}", event),
        };
    }

    fn handle_response(&mut self, _response: Response) {}

    fn handle_request(&mut self, request: Request) {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        let result = match request.command {
            Command::Known(arguments) => match arguments {
                RequestArguments::initialize(args) =>
                    self.handle_initialize(args)
                        .map(|r| ResponseBody::initialize(r)),
                RequestArguments::setBreakpoints(args) =>
                    self.handle_set_breakpoints(args)
                        .map(|r| ResponseBody::setBreakpoints(r)),
                RequestArguments::setFunctionBreakpoints(args) =>
                    self.handle_set_function_breakpoints(args)
                        .map(|r| ResponseBody::setFunctionBreakpoints(r)),
                RequestArguments::setExceptionBreakpoints(args) =>
                    self.handle_set_exception_breakpoints(args)
                        .map(|r| ResponseBody::setExceptionBreakpoints),
                RequestArguments::launch(args) => {
                    match self.handle_launch(args) {
                        Ok(responder) => {
                            self.on_configuration_done = Some((request.seq, responder));
                            return; // launch responds asynchronously
                        }
                        Err(err) => Err(err),
                    }
                }
                RequestArguments::attach(args) => {
                    match self.handle_attach(args) {
                        Ok(responder) => {
                            self.on_configuration_done = Some((request.seq, responder));
                            return; // attach responds asynchronously
                        }
                        Err(err) => Err(err),
                    }
                }
                RequestArguments::configurationDone =>
                    self.handle_configuration_done()
                        .map(|r| ResponseBody::configurationDone),
                RequestArguments::threads =>
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
                    self.handle_evaluate(args)
                        .map(|r| ResponseBody::evaluate(r)),
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
                        .map(|r| ResponseBody::next),
                RequestArguments::stepIn(args) =>
                    self.handle_step_in(args)
                        .map(|r| ResponseBody::stepIn),
                RequestArguments::stepOut(args) =>
                    self.handle_step_out(args)
                        .map(|r| ResponseBody::stepOut),
                RequestArguments::stepBack(args) =>
                    self.handle_step_back(args)
                        .map(|r| ResponseBody::stepBack),
                RequestArguments::reverseContinue(args) =>
                    self.handle_reverse_continue(args)
                        .map(|r| ResponseBody::reverseContinue),
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
                        .map(|r| ResponseBody::goto),
                RequestArguments::restartFrame(args) =>
                    self.handle_restart_frame(args)
                        .map(|r| ResponseBody::restartFrame),
                RequestArguments::dataBreakpointInfo(args) =>
                    self.handle_data_breakpoint_info(args)
                        .map(|r| ResponseBody::dataBreakpointInfo(r)),
                RequestArguments::setDataBreakpoints(args) =>
                    self.handle_set_data_breakpoints(args)
                        .map(|r| ResponseBody::setDataBreakpoints(r)),
                RequestArguments::disconnect(args) =>
                    self.handle_disconnect(Some(args))
                        .map(|_| ResponseBody::disconnect),
                RequestArguments::adapterSettings(args) =>
                    self.handle_adapter_settings(args)
                        .map(|_| ResponseBody::adapterSettings),
                _ => {
                    Err(Error::Internal("Not implemented.".into()))
                }
            },
            // A special case for DebugClient, which omits "disconnect" arguments.
            Command::Unknown { ref command } if command == "disconnect" =>
                self.handle_disconnect(None).map(|_| ResponseBody::disconnect),
            Command::Unknown { ref command } => {
                info!("Received unknown command: {}", command);
                Err(Error::Internal("Not implemented.".into()))
            }
        };
        self.send_response(request.seq, result);
    }

    fn send_response(&self, request_seq: u32, result: Result<ResponseBody, Error>) {
        let response = match result {
            Ok(body) => ProtocolMessage::Response(Response {
                request_seq: request_seq,
                success: true,
                body: Some(body),
                message: None,
                show_user: None,
            }),
            Err(err) => {
                error!("{}", err);
                ProtocolMessage::Response(Response {
                    request_seq: request_seq,
                    success: false,
                    message: Some(format!("{}", err)),
                    show_user: Some(true),
                    body: None,
                })
            }
        };
        self.send_message.borrow_mut().try_send(response).map_err(|err| error!("Could not send response: {}", err));
    }

    fn send_event(&self, event_body: EventBody) {
        let event = ProtocolMessage::Event(Event {
            seq: self.message_seq.get(),
            body: event_body,
        });
        self.message_seq.set(self.message_seq.get() + 1);
        self.send_message.borrow_mut().try_send(event).map_err(|err| error!("Could not send event: {}", err));
    }

    fn send_request(&self, args: RequestArguments) {
        let request = ProtocolMessage::Request(Request {
            seq: self.message_seq.get(),
            command: Command::Known(args),
        });
        self.message_seq.set(self.message_seq.get() + 1);
        self.send_message.borrow_mut().try_send(request).map_err(|err| error!("Could not send request: {}", err));
    }

    fn console_message(&self, output: impl Into<String>) {
        self.send_event(EventBody::output(OutputEventBody {
            output: format!("{}\n", output.into()),
            ..Default::default()
        }));
    }

    fn console_error(&self, output: impl Into<String>) {
        self.send_event(EventBody::output(OutputEventBody {
            output: format!("{}\n", output.into()),
            category: Some("stderr".into()),
            ..Default::default()
        }));
    }

    fn handle_initialize(&mut self, _args: InitializeRequestArguments) -> Result<Capabilities, Error> {
        self.event_listener.start_listening_for_event_class(&self.debugger, SBThread::broadcaster_class_name(), !0);

        let caps = Capabilities {
            supports_configuration_done_request: Some(true),
            supports_evaluate_for_hovers: Some(true),
            supports_function_breakpoints: Some(true),
            supports_conditional_breakpoints: Some(true),
            supports_hit_conditional_breakpoints: Some(true),
            supports_set_variable: Some(true),
            supports_completions_request: Some(true),
            supports_goto_targets_request: Some(true),
            supports_delayed_stack_trace_loading: Some(true),
            support_terminate_debuggee: Some(true),
            supports_log_points: Some(true),
            supports_data_breakpoints: Some(true),
            supports_restart_frame: Some(true),
            exception_breakpoint_filters: Some(self.get_exception_filters(&self.source_languages)),
            ..Default::default()
        };
        Ok(caps)
    }

    fn handle_set_breakpoints(&mut self, args: SetBreakpointsArguments) -> Result<SetBreakpointsResponseBody, Error> {
        let requested_bps = args.breakpoints.as_ref()?;
        // Decide whether this is a real source file or a disassembled range:
        // if it has a `source_reference` attribute, it's a disassembled range - we never generate references for real sources;
        // if it has an `adapter_data` attribute, it's a disassembled range from a previous debug session;
        // otherwise, it's a real source file (and we expect it to have a valid `path` attribute).
        let dasm = args
            .source
            .source_reference
            .map(|source_ref| handles::from_i64(source_ref).unwrap())
            .and_then(|source_ref| self.disassembly.find_by_handle(source_ref));

        let breakpoints = match (dasm, args.source.adapter_data, args.source.path.as_ref()) {
            (Some(dasm), _, _) => self.set_dasm_breakpoints(dasm, requested_bps),
            (None, Some(adapter_data), _) => self.set_new_dasm_breakpoints(
                &serde_json::from_value::<disassembly::AdapterData>(adapter_data)?,
                requested_bps,
            ),
            (None, None, Some(path)) => self.set_source_breakpoints(Path::new(path), requested_bps),
            _ => Err(Error::Internal(String::new())),
        }?;
        Ok(SetBreakpointsResponseBody {
            breakpoints,
        })
    }

    fn set_source_breakpoints(
        &mut self,
        file_path: &Path,
        requested_bps: &[SourceBreakpoint],
    ) -> Result<Vec<Breakpoint>, Error> {
        let BreakpointsState {
            ref mut source,
            ref mut breakpoint_infos,
            ..
        } = *self.breakpoints.borrow_mut();

        let file_path_norm = normalize_path(file_path);
        let existing_bps = source.entry(file_path.into()).or_default();
        let mut new_bps = HashMap::new();
        let mut result = vec![];
        for req in requested_bps {
            // Find existing breakpoint or create a new one
            let bp = match existing_bps.get(&req.line).and_then(|bp_id| self.target.find_breakpoint_by_id(*bp_id)) {
                Some(bp) => bp,
                None => self.target.breakpoint_create_by_location(file_path_norm.to_str()?, req.line as u32),
            };

            let bp_info = BreakpointInfo {
                id: bp.id(),
                breakpoint: bp,
                kind: BreakpointKind::Location,
                condition: req.condition.clone(),
                log_message: req.log_message.clone(),
                hit_condition: self.parse_hit_condition(req.hit_condition.as_ref()),
                hit_count: 0,
            };

            self.init_bp_actions(&bp_info);
            result.push(self.make_bp_response(&bp_info));
            new_bps.insert(req.line, bp_info.id);
            breakpoint_infos.insert(bp_info.id, bp_info);
        }
        for (line, bp_id) in existing_bps.iter() {
            if !new_bps.contains_key(line) {
                self.target.breakpoint_delete(*bp_id);
                breakpoint_infos.remove(bp_id);
            }
        }
        mem::replace(existing_bps, new_bps);
        Ok(result)
    }

    fn set_dasm_breakpoints(
        &mut self,
        dasm: Rc<disassembly::DisassembledRange>,
        requested_bps: &[SourceBreakpoint],
    ) -> Result<Vec<Breakpoint>, Error> {
        let BreakpointsState {
            ref mut assembly,
            ref mut breakpoint_infos,
            ..
        } = *self.breakpoints.borrow_mut();
        let existing_bps = assembly.entry(dasm.handle()).or_default();
        let mut new_bps = HashMap::new();
        let mut result = vec![];
        for req in requested_bps {
            let laddress = dasm.address_by_line_num(req.line as u32);

            // Find existing breakpoint or create a new one
            let bp = match existing_bps.get(&req.line).and_then(|bp_id| self.target.find_breakpoint_by_id(*bp_id)) {
                Some(bp) => bp,
                None => self.target.breakpoint_create_by_absolute_address(laddress),
            };

            let bp_info = BreakpointInfo {
                id: bp.id(),
                breakpoint: bp,
                kind: BreakpointKind::Address,
                condition: req.condition.clone(),
                log_message: req.log_message.clone(),
                hit_condition: self.parse_hit_condition(req.hit_condition.as_ref()),
                hit_count: 0,
            };
            self.init_bp_actions(&bp_info);
            result.push(self.make_bp_response(&bp_info));
            new_bps.insert(req.line, bp_info.id);
            breakpoint_infos.insert(bp_info.id, bp_info);
        }
        for (line, bp_id) in existing_bps.iter() {
            if !new_bps.contains_key(line) {
                self.target.breakpoint_delete(*bp_id);
            }
        }
        mem::replace(existing_bps, new_bps);
        Ok(result)
    }

    fn set_new_dasm_breakpoints(
        &mut self,
        adapter_data: &disassembly::AdapterData,
        requested_bps: &[SourceBreakpoint],
    ) -> Result<Vec<Breakpoint>, Error> {
        let mut new_bps = HashMap::new();
        let mut result = vec![];
        let line_addresses = disassembly::DisassembledRange::lines_from_adapter_data(adapter_data);
        for req in requested_bps {
            let address = line_addresses[req.line as usize] as Address;
            let bp = self.target.breakpoint_create_by_absolute_address(address);
            let bp_info = BreakpointInfo {
                id: bp.id(),
                breakpoint: bp,
                kind: BreakpointKind::Address,
                condition: req.condition.clone(),
                log_message: req.log_message.clone(),
                hit_condition: self.parse_hit_condition(req.hit_condition.as_ref()),
                hit_count: 0,
            };
            self.init_bp_actions(&bp_info);
            result.push(Breakpoint {
                id: Some(bp_info.id as i64),
                ..Default::default()
            });
            new_bps.insert(req.line, bp_info.id);
            self.breakpoints.get_mut().breakpoint_infos.insert(bp_info.id, bp_info);
        }
        Ok(result)
    }

    // Generates debug_protocol::Breakpoint message from BreakpointInfo
    fn make_bp_response(&self, bp_info: &BreakpointInfo) -> Breakpoint {
        let message = Some(format!("Locations: {}", bp_info.breakpoint.num_locations()));

        if bp_info.breakpoint.num_locations() == 0 {
            Breakpoint {
                id: Some(bp_info.id as i64),
                verified: false,
                message,
                ..Default::default()
            }
        } else {
            match &bp_info.kind {
                BreakpointKind::Location => {
                    let address = bp_info.breakpoint.location_at_index(0).address();
                    if let Some(le) = address.line_entry() {
                        let file_path = le.file_spec().path();
                        Breakpoint {
                            id: Some(bp_info.id as i64),
                            source: Some(Source {
                                name: Some(file_path.file_name().unwrap().to_string_lossy().into_owned()),
                                path: Some(file_path.as_os_str().to_string_lossy().into_owned()),
                                ..Default::default()
                            }),
                            line: Some(le.line() as i64),
                            verified: bp_info.breakpoint.num_locations() > 0,
                            message,
                            ..Default::default()
                        }
                    } else {
                        Breakpoint {
                            id: Some(bp_info.id as i64),
                            verified: false,
                            message,
                            ..Default::default()
                        }
                    }
                }
                BreakpointKind::Address => {
                    let address = bp_info.breakpoint.location_at_index(0).address();
                    let laddress = address.load_address(&self.target);
                    let dasm = self.disassembly.find_by_address(laddress).unwrap();
                    let adapter_data = Some(serde_json::to_value(dasm.adapter_data()).unwrap());
                    Breakpoint {
                        id: Some(bp_info.id as i64),
                        verified: true,
                        line: Some(dasm.line_num_by_address(laddress) as i64),
                        source: Some(Source {
                            name: Some(dasm.source_name().to_owned()),
                            source_reference: Some(handles::to_i64(Some(dasm.handle()))),
                            adapter_data: adapter_data,
                            ..Default::default()
                        }),
                        message,
                        ..Default::default()
                    }
                }
                BreakpointKind::Function => Breakpoint {
                    id: Some(bp_info.id as i64),
                    verified: bp_info.breakpoint.num_locations() > 0,
                    message,
                    ..Default::default()
                },
                BreakpointKind::Exception => Breakpoint {
                    id: Some(bp_info.id as i64),
                    verified: bp_info.breakpoint.num_locations() > 0,
                    message,
                    ..Default::default()
                },
            }
        }
    }

    fn parse_hit_condition(&self, expr: Option<&String>) -> Option<HitCondition> {
        if let Some(expr) = expr {
            let expr = expr.trim();
            if !expr.is_empty() {
                match expressions::parse_hit_condition(&expr) {
                    Ok(cond) => Some(cond),
                    Err(_) => {
                        self.console_error(format!("Invalid hit condition: {}", expr));
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn handle_set_function_breakpoints(
        &mut self,
        args: SetFunctionBreakpointsArguments,
    ) -> Result<SetBreakpointsResponseBody, Error> {
        let BreakpointsState {
            ref mut function,
            ref mut breakpoint_infos,
            ..
        } = *self.breakpoints.borrow_mut();
        let mut new_bps = HashMap::new();
        let mut result = vec![];
        for req in args.breakpoints {
            // Find existing breakpoint or create a new one
            let bp = match function.get(&req.name).and_then(|bp_id| self.target.find_breakpoint_by_id(*bp_id)) {
                Some(bp) => bp,
                None => {
                    if req.name.starts_with("/re ") {
                        self.target.breakpoint_create_by_regex(&req.name[4..])
                    } else {
                        self.target.breakpoint_create_by_name(&req.name)
                    }
                }
            };

            let bp_info = BreakpointInfo {
                id: bp.id(),
                breakpoint: bp,
                kind: BreakpointKind::Function,
                condition: req.condition,
                log_message: None,
                hit_condition: self.parse_hit_condition(req.hit_condition.as_ref()),
                hit_count: 0,
            };
            self.init_bp_actions(&bp_info);
            result.push(self.make_bp_response(&bp_info));
            new_bps.insert(req.name, bp_info.id);
            breakpoint_infos.insert(bp_info.id, bp_info);
        }
        for (name, bp_id) in function.iter() {
            if !new_bps.contains_key(name) {
                self.target.breakpoint_delete(*bp_id);
            }
        }
        mem::replace(function, new_bps);

        Ok(SetBreakpointsResponseBody {
            breakpoints: result,
        })
    }

    fn handle_set_exception_breakpoints(&mut self, args: SetExceptionBreakpointsArguments) -> Result<(), Error> {
        let mut breakpoints = self.breakpoints.borrow_mut();
        breakpoints.breakpoint_infos.retain(|id, bp_info| {
            if let BreakpointKind::Exception = bp_info.kind {
                self.target.breakpoint_delete(bp_info.id);
                false
            } else {
                true
            }
        });
        drop(breakpoints);

        for bp in self.set_exception_breakpoints(&args.filters) {
            let bp_info = BreakpointInfo {
                id: bp.id(),
                breakpoint: bp,
                kind: BreakpointKind::Exception,
                condition: None,
                log_message: None,
                hit_condition: None,
                hit_count: 0,
            };
            self.breakpoints.borrow_mut().breakpoint_infos.insert(bp_info.id, bp_info);
        }
        Ok(())
    }

    fn get_exception_filters(&self, source_langs: &[String]) -> Vec<ExceptionBreakpointsFilter> {
        let mut filters = vec![];
        if source_langs.iter().any(|x| x == "cpp") {
            filters.push(ExceptionBreakpointsFilter {
                filter: "cpp_throw".into(),
                label: "C++: on throw".into(),
                default: Some(true),
            });
            filters.push(ExceptionBreakpointsFilter {
                filter: "cpp_catch".into(),
                label: "C++: on catch".into(),
                default: Some(false),
            });
        }
        if source_langs.iter().any(|x| x == "rust") {
            filters.push(ExceptionBreakpointsFilter {
                filter: "rust_panic".into(),
                label: "Rust: on panic".into(),
                default: Some(true),
            });
        }
        filters
    }

    fn set_exception_breakpoints(&mut self, filters: &[String]) -> Vec<SBBreakpoint> {
        let cpp_throw = filters.iter().any(|x| x == "cpp_throw");
        let cpp_catch = filters.iter().any(|x| x == "cpp_catch");
        let rust_panic = filters.iter().any(|x| x == "rust_panic");
        let mut bps = vec![];
        if cpp_throw || cpp_catch {
            bps.push(self.target.breakpoint_create_for_exception(LanguageType::C_plus_plus, cpp_catch, cpp_throw));
        }
        if rust_panic {
            bps.push(self.target.breakpoint_create_by_name("rust_panic"));
        }
        bps
    }

    fn init_bp_actions(&self, bp_info: &BreakpointInfo) {
        // Determine conditional expression type:
        let py_condition = if let Some(ref condition) = bp_info.condition {
            let condition = condition.trim();
            if !condition.is_empty() {
                let pp_expr = expressions::prepare(condition, self.default_expr_type);
                match pp_expr {
                    // if native, use that directly,
                    PreparedExpression::Native(expr) => {
                        bp_info.breakpoint.set_condition(&expr);
                        None
                    }
                    // otherwise, we'll need to evaluate it ourselves in the breakpoint callback.
                    _ => Some(pp_expr),
                }
            } else {
                None
            }
        } else {
            None
        };

        let hit_condition = bp_info.hit_condition.clone();

        let self_ref = self.self_ref.clone();
        bp_info.breakpoint.set_callback(move |process, thread, location| {
            debug!("Callback for breakpoint location {:?}", location);
            if let Some(self_ref) = self_ref.upgrade() {
                let mut session = self_ref.lock().unwrap();
                session.on_breakpoint_hit(process, thread, location, &py_condition, &hit_condition)
            } else {
                false // Can't upgrade weak ref to strong - the session must already be gone.  Don't stop.
            }
        });
    }

    fn on_breakpoint_hit(
        &self,
        _process: &SBProcess,
        thread: &SBThread,
        location: &SBBreakpointLocation,
        py_condition: &Option<PreparedExpression>,
        hit_condition: &Option<HitCondition>,
    ) -> bool {
        let mut breakpoints = self.breakpoints.borrow_mut();
        let bp_info = breakpoints.breakpoint_infos.get_mut(&location.breakpoint().id()).unwrap();

        if let Some(pp_expr) = py_condition {
            let (pycode, is_simple_expr) = match pp_expr {
                PreparedExpression::Simple(expr) => (expr, true),
                PreparedExpression::Python(expr) => (expr, false),
                PreparedExpression::Native(_) => unreachable!(),
            };
            let frame = thread.frame_at_index(0);
            let context = self.context_from_frame(Some(&frame));
            // TODO: pass bpno
            let should_stop = match &self.python {
                Some(python) => match python.evaluate_as_bool(&pycode, is_simple_expr, &context) {
                    Ok(val) => val,
                    Err(err) => {
                        self.console_error(err.to_string());
                        return true; // Stop on evluation errors, even if there's a log message.
                    }
                },
                None => {
                    return true;
                }
            };

            if !should_stop {
                return false;
            }
        }

        // We maintain our own hit count for consistency between native and python conditions:
        // LLDB doesn't count breakpoint hits for which native condition evaluated to false,
        // however it does count ones where the callback was invoked, even if it returned false.
        bp_info.hit_count += 1;

        if let Some(hit_condition) = hit_condition {
            let hit_count = bp_info.hit_count;
            let should_stop = match hit_condition {
                HitCondition::LT(n) => hit_count < *n,
                HitCondition::LE(n) => hit_count <= *n,
                HitCondition::EQ(n) => hit_count == *n,
                HitCondition::GE(n) => hit_count >= *n,
                HitCondition::GT(n) => hit_count > *n,
                HitCondition::MOD(n) => hit_count % *n == 0,
            };
            if !should_stop {
                return false;
            }
        }

        // If we are supposed to stop and there's a log message, evaluate and print the message, but don't stop.
        if let Some(ref log_message) = bp_info.log_message {
            let frame = thread.frame_at_index(0);
            let message = self.format_logpoint_message(log_message, &frame);
            self.console_message(message);
            return false;
        }

        true
    }

    // Replaces {expression}'s in log_message with results of their evaluations.
    fn format_logpoint_message(&self, log_message: &str, frame: &SBFrame) -> String {
        // Finds expressions ({...}) in message and invokes the callback on them.
        fn replace_logpoint_expressions<F>(message: &str, f: F) -> String
        where
            F: Fn(&str) -> Result<String, String>,
        {
            let mut start = 0;
            let mut nesting = 0;
            let mut result = String::new();
            for (idx, ch) in message.char_indices() {
                if ch == '{' {
                    if nesting == 0 {
                        result.push_str(&message[start..idx]);
                        start = idx + 1;
                    }
                    nesting += 1;
                } else if ch == '}' && nesting > 0 {
                    nesting -= 1;
                    if nesting == 0 {
                        let str_val = match f(&message[start..idx]) {
                            Ok(ok) => ok,
                            Err(err) => format!("{{Error: {}}}", err),
                        };
                        result.push_str(&str_val);
                        start = idx + 1;
                    }
                }
            }
            result.push_str(&message[start..(message.len())]);
            result
        }

        replace_logpoint_expressions(&log_message, |expr| {
            let (pp_expr, expr_format) = expressions::prepare_with_format(expr, self.default_expr_type)?;
            let format = match expr_format {
                None | Some(FormatSpec::Array(_)) => self.global_format,
                Some(FormatSpec::Format(format)) => format,
            };
            let sbval = self.evaluate_expr_in_frame(&pp_expr, Some(frame)).map_err(|err| err.to_string())?;
            let str_val = self.get_var_value_str(&sbval, format, sbval.num_children() > 0);
            Ok(str_val)
        })
    }

    // Invoke f() on session's main thread
    fn invoke_on_main_thread<F, R>(self_ref: &Arc<Mutex<Self>>, f: F) -> R
    where
        F: FnOnce() -> R + Send,
        R: Send + 'static,
    {
        let (sender, receiver) = std::sync::mpsc::channel::<R>();
        let cb: Box<dyn FnOnce() + Send> = Box::new(move || sender.send(f()).unwrap());
        // Casting away cb's lifetime.
        // This is safe, because we are blocking current thread until f() returns.
        let cb: Box<dyn FnOnce() + Send + 'static> = unsafe { std::mem::transmute(cb) };
        self_ref.lock().unwrap().incoming_send.send(InputEvent::Invoke(cb)).unwrap();
        receiver.recv().unwrap()
    }

    fn handle_launch(&mut self, args: LaunchRequestArguments) -> Result<Box<AsyncResponder>, Error> {
        self.common_init_session(&args.common)?;

        if let Some(true) = &args.custom {
            self.handle_custom_launch(args)
        } else {
            let program = match &args.program {
                Some(program) => program,
                None => return Err(Error::UserError("\"program\" property is required for launch".into())),
            };
            self.target = Initialized(self.create_target_from_program(program)?);
            self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));
            self.send_event(EventBody::initialized);
            Ok(Box::new(move |s: &mut DebugSession| s.complete_launch(args)))
        }
    }

    fn complete_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        let mut launch_info = self.target.launch_info();

        // Merge environment
        let mut launch_env = HashMap::new();
        for (k, v) in env::vars() {
            launch_env.insert(k, v);
        }
        if let Some(ref env) = args.env {
            for (k, v) in env.iter() {
                launch_env.insert(k.clone(), v.clone());
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
        self.configure_stdio(&args, &mut launch_info);
        self.target.set_launch_info(&launch_info);

        // Run user commands (which may modify launch info)
        if let Some(ref commands) = args.common.pre_run_commands {
            self.exec_commands("preRunCommands", commands)?;
        }
        // Grab updated launch info.
        let mut launch_info = self.target.launch_info();

        // Announce the final launch command line
        let executable = self.target.executable().path().to_string_lossy().into_owned();
        let command_line = launch_info.arguments().fold(executable, |mut args, a| {
            args.push(' ');
            args.push_str(a);
            args
        });
        self.console_message(format!("Launching: {}", command_line));

        if args.no_debug.unwrap_or(false) {
            // No-debug launch: start debuggee directly and terminate debug session.
            launch_info.set_executable_file(&self.target.executable(), true);
            let status = match &self.debuggee_terminal {
                Some(t) => t.attach(|| self.target.platform().launch(&launch_info)),
                None => self.target.platform().launch(&launch_info),
            };
            // Terminate debug session
            self.send_event(EventBody::terminated(TerminatedEventBody {
                restart: None,
            }));
            match status.into_result() {
                Ok(()) => Ok(ResponseBody::launch),
                Err(err) => Err(Error::UserError(err.error_string().into())),
            }
        } else {
            // Normal launch
            launch_info.set_listener(&self.event_listener);

            let result = match &self.debuggee_terminal {
                Some(t) => t.attach(|| self.target.launch(&launch_info)),
                None => self.target.launch(&launch_info),
            };

            let process = match result {
                Ok(process) => process,
                Err(err) => {
                    let mut msg: String = err.error_string().into();
                    if let Some(work_dir) = launch_info.working_directory() {
                        if self.target.platform().get_file_permissions(work_dir) == 0 {
                            msg = format!(
                                "{}\n\nPossible cause: the working directory \"{}\" is missing or inaccessible.",
                                msg,
                                work_dir.display()
                            );
                        }
                    }
                    return Err(Error::UserError(msg));
                }
            };
            self.process = Initialized(process);
            self.process_was_launched = true;

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
    }

    fn handle_custom_launch(&mut self, args: LaunchRequestArguments) -> Result<Box<AsyncResponder>, Error> {
        if let Some(commands) = &args.target_create_commands {
            self.exec_commands("targetCreateCommands", &commands)?;
        }
        self.target = Initialized(self.debugger.selected_target());
        self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));
        self.send_event(EventBody::initialized);
        Ok(Box::new(move |s: &mut DebugSession| s.complete_custom_launch(args)))
    }

    fn complete_custom_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        if let Some(commands) = args.process_create_commands.as_ref().or(args.common.pre_run_commands.as_ref()) {
            self.exec_commands("processCreateCommands", &commands)?;
        }
        self.process = Initialized(self.target.process());
        self.process.broadcaster().add_listener(&self.event_listener, !0);
        self.process_was_launched = true;

        // This is succeptible to race conditions, but probably the best we can do.
        if self.process.state().is_stopped() {
            self.notify_process_stopped();
        }

        self.exit_commands = args.common.exit_commands;
        Ok(ResponseBody::launch)
    }

    fn handle_attach(&mut self, args: AttachRequestArguments) -> Result<Box<AsyncResponder>, Error> {
        self.common_init_session(&args.common)?;

        if args.program.is_none() && args.pid.is_none() {
            return Err(Error::UserError(r#"Either "program" or "pid" is required for attach."#.into()));
        }

        self.target = Initialized(self.debugger.create_target("", None, None, false)?);
        self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));
        self.send_event(EventBody::initialized);
        Ok(Box::new(move |s: &mut DebugSession| s.complete_attach(args)))
    }

    fn complete_attach(&mut self, args: AttachRequestArguments) -> Result<ResponseBody, Error> {
        if let Some(ref commands) = args.common.pre_run_commands {
            self.exec_commands("preRunCommands", commands)?;
        }

        let attach_info = SBAttachInfo::new();
        if let Some(pid) = args.pid {
            let pid = match pid {
                Pid::Number(n) => n as ProcessID,
                Pid::String(s) => {
                    s.parse().map_err(|_| Error::UserError("Process id must me a positive integer.".into()))?
                }
            };
            attach_info.set_process_id(pid);
        } else if let Some(program) = args.program {
            attach_info.set_executable(&self.find_executable(&program));
        } else {
            unreachable!()
        }
        attach_info.set_wait_for_launch(args.wait_for.unwrap_or(false), false);
        attach_info.set_ignore_existing(false);
        attach_info.set_listener(&self.event_listener);

        let process = match self.target.attach(&attach_info) {
            Ok(process) => process,
            Err(err) => return Err(Error::UserError(err.error_string().into())),
        };
        self.process = Initialized(process);
        self.process_was_launched = false;

        if args.common.stop_on_entry.unwrap_or(false) {
            self.notify_process_stopped();
        } else {
            self.process.resume();
        }

        if let Some(commands) = args.common.post_run_commands {
            self.exec_commands("postRunCommands", &commands)?;
        }
        self.exit_commands = args.common.exit_commands;
        Ok(ResponseBody::attach)
    }

    fn create_target_from_program(&self, program: &str) -> Result<SBTarget, Error> {
        let target = match self.debugger.create_target(program, None, None, false) {
            Ok(target) => target,
            // TODO: use selected platform instead of cfg!(windows)
            Err(_) if cfg!(windows) && !program.ends_with(".exe") => {
                let program = format!("{}.exe", program);
                match self.debugger.create_target(&program, None, None, false) {
                    Ok(target) => target,
                    Err(err) => return Err(err.into()),
                }
            }
            Err(err) => return Err(err.into()),
        };
        Ok(target)
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

    fn configure_stdio(&mut self, args: &LaunchRequestArguments, launch_info: &mut SBLaunchInfo) -> Result<(), Error> {
        let terminal_kind = match args.terminal.unwrap_or(TerminalKind::Integrated) {
            TerminalKind::Console => None,
            TerminalKind::External => Some("external"),
            TerminalKind::Integrated => Some("integrated"),
        };

        if let Some(terminal_kind) = terminal_kind {
            let title = args.common.name.clone().unwrap_or_else(|| "Debug".into());
            let result = Terminal::create(|agent_args| {
                let req_args = RunInTerminalRequestArguments {
                    args: agent_args,
                    cwd: String::new(),
                    env: None,
                    kind: Some(terminal_kind.to_owned()),
                    title: Some(title),
                };
                self.send_request(RequestArguments::runInTerminal(req_args));
                Ok(())
            });

            match result {
                Ok(terminal) => self.debuggee_terminal = Some(terminal),
                Err(err) => self.console_error(format!(
                    "Failed to redirect stdio to a terminal. ({})\nDebuggee output will appear here.",
                    err
                )),
            }
        }

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
                    "Could not initialize Python interpreter - only native expressions will be available.",
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
            self.update_adapter_settings(settings);
        }

        if let Some(commands) = &args_common.init_commands {
            self.exec_commands("initCommands", &commands)?;
        }

        Ok(())
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
                return Err(Error::UserError(err));
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
        info!("Set target.source-map args: {}", args);
        self.debugger.set_variable("target.source-map", &args);
    }

    fn handle_configuration_done(&mut self) -> Result<(), Error> {
        self.target.broadcaster().add_listener(
            &self.event_listener,
            SBTargetEvent::BroadcastBitBreakpointChanged | SBTargetEvent::BroadcastBitModulesLoaded,
        );
        if let Some((request_seq, responder)) = self.on_configuration_done.take() {
            let result = responder.call_once((self,));
            self.send_response(request_seq, result);
        }
        Ok(())
    }

    fn handle_threads(&mut self) -> Result<ThreadsResponseBody, Error> {
        if !self.process.is_initialized() {
            // VSCode may send a `threads` request after a failed launch.
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
                write!(descr, " \"{}\"", name);
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
                return Err(Error::Protocol("Invalid thread id.".into()));
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
                let dasm = self.disassembly.get_by_address(pc_addr);
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

    fn handle_scopes(&mut self, args: ScopesArguments) -> Result<ScopesResponseBody, Error> {
        let frame_id = Handle::new(args.frame_id as u32).unwrap();
        if let Some(Container::StackFrame(frame)) = self.var_refs.get(frame_id) {
            let frame = frame.clone();
            let locals_handle = self.var_refs.create(Some(frame_id), "[locs]", Container::Locals(frame.clone()));
            let locals = Scope {
                name: "Local".into(),
                variables_reference: locals_handle.get() as i64,
                expensive: false,
                ..Default::default()
            };
            let statics_handle = self.var_refs.create(Some(frame_id), "[stat]", Container::Statics(frame.clone()));
            let statics = Scope {
                name: "Static".into(),
                variables_reference: statics_handle.get() as i64,
                expensive: false,
                ..Default::default()
            };
            let globals_handle = self.var_refs.create(Some(frame_id), "[glob]", Container::Globals(frame.clone()));
            let globals = Scope {
                name: "Global".into(),
                variables_reference: globals_handle.get() as i64,
                expensive: false,
                ..Default::default()
            };
            let registers_handle = self.var_refs.create(Some(frame_id), "[regs]", Container::Registers(frame));
            let registers = Scope {
                name: "Registers".into(),
                variables_reference: registers_handle.get() as i64,
                expensive: false,
                ..Default::default()
            };
            Ok(ScopesResponseBody {
                scopes: vec![locals, statics, globals, registers],
            })
        } else {
            Err(Error::Internal(format!("Invalid frame reference: {}", args.frame_id)))
        }
    }

    fn handle_variables(&mut self, args: VariablesArguments) -> Result<VariablesResponseBody, Error> {
        let container_handle = handles::from_i64(args.variables_reference)?;

        if let Some(container) = self.var_refs.get(container_handle) {
            let variables = match container {
                Container::Locals(frame) => {
                    let ret_val = frame.thread().stop_return_value();
                    let variables = frame.variables(&VariableOptions {
                        arguments: true,
                        locals: true,
                        statics: false,
                        in_scope_only: true,
                    });
                    let mut vars_iter = variables.iter();
                    let mut variables = self.convert_scope_values(&mut vars_iter, "", Some(container_handle));
                    // Prepend last function return value, if any.
                    if let Some(ret_val) = ret_val {
                        let mut variable = self.var_to_variable(&ret_val, "", Some(container_handle));
                        variable.name = "[return value]".to_owned();
                        variables.insert(0, variable);
                    }
                    variables
                }
                Container::Statics(frame) => {
                    let variables = frame.variables(&VariableOptions {
                        arguments: false,
                        locals: false,
                        statics: true,
                        in_scope_only: true,
                    });
                    let mut vars_iter = variables.iter().filter(|v| v.value_type() != ValueType::VariableStatic);
                    self.convert_scope_values(&mut vars_iter, "", Some(container_handle))
                }
                Container::Globals(frame) => {
                    let variables = frame.variables(&VariableOptions {
                        arguments: false,
                        locals: false,
                        statics: true,
                        in_scope_only: true,
                    });
                    let mut vars_iter = variables.iter(); //.filter(|v| v.value_type() != ValueType::VariableGlobal);
                    self.convert_scope_values(&mut vars_iter, "", Some(container_handle))
                }
                Container::Registers(frame) => {
                    let list = frame.registers();
                    let mut vars_iter = list.iter();
                    self.convert_scope_values(&mut vars_iter, "", Some(container_handle))
                }
                Container::SBValue(var) => {
                    let container_eval_name = self.compose_container_eval_name(container_handle);
                    let var = var.clone();
                    let mut vars_iter = var.children();
                    let mut variables =
                        self.convert_scope_values(&mut vars_iter, &container_eval_name, Some(container_handle));
                    // If synthetic, add [raw] view.
                    if var.is_synthetic() {
                        let raw_var = var.non_synthetic_value();
                        let handle = self.var_refs.create(Some(container_handle), "[raw]", Container::SBValue(raw_var));
                        let raw = Variable {
                            name: "[raw]".to_owned(),
                            value: var.type_name().unwrap_or_default().to_owned(),
                            variables_reference: handles::to_i64(Some(handle)),
                            ..Default::default()
                        };
                        variables.push(raw);
                    }
                    variables
                }
                Container::StackFrame(_) => vec![],
            };
            Ok(VariablesResponseBody {
                variables: variables,
            })
        } else {
            Err(Error::Internal(format!("Invalid variabes reference: {}", container_handle)))
        }
    }

    fn compose_container_eval_name(&self, container_handle: Handle) -> String {
        let mut eval_name = String::new();
        let mut container_handle = Some(container_handle);
        while let Some(h) = container_handle {
            let (parent_handle, key, value) = self.var_refs.get_full_info(h).unwrap();
            match value {
                Container::SBValue(var) if var.value_type() != ValueType::RegisterSet => {
                    eval_name = compose_eval_name(key, eval_name);
                    container_handle = parent_handle;
                }
                _ => break,
            }
        }
        eval_name
    }

    fn convert_scope_values(
        &mut self,
        vars_iter: &mut dyn Iterator<Item = SBValue>,
        container_eval_name: &str,
        container_handle: Option<Handle>,
    ) -> Vec<Variable> {
        let mut variables = vec![];
        let mut variables_idx = HashMap::new();

        let start = time::SystemTime::now();
        for var in vars_iter {
            let variable = self.var_to_variable(&var, container_eval_name, container_handle);

            // Ensure proper shadowing
            if let Some(idx) = variables_idx.get(&variable.name) {
                variables[*idx] = variable;
            } else {
                variables_idx.insert(variable.name.clone(), variables.len());
                variables.push(variable);
            }

            // Bail out if timeout has expired.
            if start.elapsed().unwrap_or_default() > self.evaluation_timeout {
                self.console_error("Child list expansion has timed out.");
                variables.push(Variable {
                    name: "[timed out]".to_owned(),
                    type_: Some("Expansion of this list has timed out.".to_owned()),
                    ..Default::default()
                });
                break;
            }
        }
        variables
    }

    // SBValue to VSCode Variable
    fn var_to_variable(
        &mut self,
        var: &SBValue,
        container_eval_name: &str,
        container_handle: Option<Handle>,
    ) -> Variable {
        let name = var.name().unwrap_or_default();
        let dtype = var.type_name();
        let value = self.get_var_value_str(&var, self.global_format, container_handle.is_some());
        let handle = self.get_var_handle(container_handle, name, &var);

        let eval_name = if var.prefer_synthetic_value() {
            Some(compose_eval_name(container_eval_name, name))
        } else {
            var.expression_path().map(|p| {
                let mut p = p;
                p.insert_str(0, "/nat ");
                p
            })
        };

        Variable {
            name: name.to_owned(),
            value: value,
            type_: dtype.map(|v| v.to_owned()),
            variables_reference: handles::to_i64(handle),
            evaluate_name: eval_name,
            ..Default::default()
        }
    }

    // Generate a handle for a variable.
    fn get_var_handle(&mut self, parent_handle: Option<Handle>, key: &str, var: &SBValue) -> Option<Handle> {
        if var.num_children() > 0 || var.is_synthetic() {
            Some(self.var_refs.create(parent_handle, key, Container::SBValue(var.clone())))
        } else {
            None
        }
    }

    // Get displayable string from an SBValue
    fn get_var_value_str(&self, var: &SBValue, format: Format, is_container: bool) -> String {
        let err = var.error();
        if err.is_failure() {
            return format!("<{}>", err);
        }

        let mut var = Cow::Borrowed(var);
        var.set_format(format);

        if self.deref_pointers && format == Format::Default {
            let ptr_type = var.type_();
            let type_class = ptr_type.type_class();
            if type_class.intersects(TypeClass::Pointer | TypeClass::Reference) {
                // If the pointer has an associated synthetic, or if it's a pointer to a basic
                // type such as `char`, use summary of the pointer itself;
                // otherwise prefer to dereference and use summary of the pointee.
                let pointee_basic_type = ptr_type.pointee_type().basic_type();
                if var.is_synthetic() || pointee_basic_type != BasicType::Invalid {
                    if let Some(value_str) = var.summary().map(|s| into_string_lossy(s)) {
                        return value_str;
                    }
                }

                // try dereferencing
                let pointee = var.dereference();
                let pointee_type_size = pointee.type_().byte_size() as usize;
                // If pointee is valid, and data can be read,
                if pointee.is_valid() && pointee_type_size == pointee.data().byte_size() {
                    var = Cow::Owned(pointee);
                } else {
                    if var.value_as_unsigned(0) == 0 {
                        return "<null>".to_owned();
                    } else if pointee_type_size > 0 {
                        return "<invalid address>".to_owned();
                    }
                }
            }
        }

        // Try value,
        if let Some(value_str) = var.value().map(|s| into_string_lossy(s)) {
            return value_str;
        }
        // ...then try summary
        if let Some(summary_str) = var.summary().map(|s| into_string_lossy(s)) {
            return summary_str;
        }

        if is_container {
            self.get_container_summary(var.as_ref())
        } else {
            "<not available>".to_owned()
        }
    }

    fn get_container_summary(&self, var: &SBValue) -> String {
        const MAX_LENGTH: usize = 32;

        let mut summary = String::from("{");
        let mut empty = true;
        for child in var.children() {
            if summary.len() > MAX_LENGTH {
                summary.push_str(", ...");
                break;
            }

            if let Some(name) = child.name() {
                if let Some(Ok(value)) = child.value().map(|s| s.to_str()) {
                    if empty {
                        empty = false;
                    } else {
                        summary.push_str(", ");
                    }

                    if name.starts_with("[") {
                        summary.push_str(value);
                    } else {
                        write!(summary, "{}:{}", name, value);
                    }
                }
            }
        }

        if empty {
            summary.push_str("...");
        }
        summary.push_str("}");
        summary
    }

    fn handle_evaluate(&mut self, args: EvaluateArguments) -> Result<EvaluateResponseBody, Error> {
        let frame = if let Some(frame_id) = args.frame_id {
            let handle = handles::from_i64(frame_id)?;
            let frame = match self.var_refs.get(handle) {
                Some(Container::StackFrame(ref f)) => f.clone(),
                _ => return Err(Error::Internal("Invalid frameId".into())),
            };
            // If they used `frame select` command in after the last stop, use currently selected frame
            // from frame's thread, instead of the frame itself.
            if self.selected_frame_changed {
                let thread = frame.thread();
                Some(thread.selected_frame())
            } else {
                Some(frame)
            }
        } else {
            None
        };

        let context = args.context.as_ref().map(|s| s.as_ref());
        if let Some("repl") = context {
            match self.console_mode {
                ConsoleMode::Commands => {
                    if args.expression.starts_with("?") {
                        self.handle_evaluate_expression(&args.expression[1..], frame)
                    } else {
                        self.handle_execute_command(&args.expression, frame)
                    }
                }
                ConsoleMode::Evaluate => {
                    if args.expression.starts_with('`') {
                        self.handle_execute_command(&args.expression[1..], frame)
                    } else {
                        self.handle_evaluate_expression(&args.expression, frame)
                    }
                }
            }
        } else {
            self.handle_evaluate_expression(&args.expression, frame)
        }
    }

    fn handle_execute_command(&mut self, command: &str, frame: Option<SBFrame>) -> Result<EvaluateResponseBody, Error> {
        let context = self.context_from_frame(frame.as_ref());
        let mut result = SBCommandReturnObject::new();
        let interp = self.debugger.command_interpreter();
        let ok = interp.handle_command_with_context(command, &context, &mut result, false);
        debug!("{} -> {:?}, {:?}", command, ok, result);
        // TODO: multiline
        let text = if result.succeeded() {
            result.output()
        } else {
            result.error()
        };
        let response = EvaluateResponseBody {
            result: into_string_lossy(text),
            ..Default::default()
        };
        return Ok(response);
    }

    fn handle_evaluate_expression(
        &mut self,
        expression: &str,
        frame: Option<SBFrame>,
    ) -> Result<EvaluateResponseBody, Error> {
        // Expression
        let (pp_expr, expr_format) = expressions::prepare_with_format(expression, self.default_expr_type)
            .map_err(|err| Error::UserError(err))?;

        match self.evaluate_expr_in_frame(&pp_expr, frame.as_ref()) {
            Ok(mut sbval) => {
                let (var, format) = match expr_format {
                    None => (sbval, self.global_format),
                    Some(FormatSpec::Format(format)) => (sbval, format),
                    // Interpret as array of `size` elements:
                    Some(FormatSpec::Array(size)) => {
                        let var_type = sbval.type_();
                        let type_class = var_type.type_class();
                        let var = if type_class.intersects(TypeClass::Pointer | TypeClass::Reference) {
                            // For pointers and references we re-interpret the pointee.
                            let array_type = var_type.pointee_type().array_type(size as u64);
                            let addr = sbval.dereference().address().unwrap();
                            sbval.target().create_value_from_address("(as array)", &addr, &array_type)
                        } else if type_class.intersects(TypeClass::Array) {
                            // For arrays, re-interpret the array length.
                            let array_type = var_type.array_element_type().array_type(size as u64);
                            let addr = sbval.address().unwrap();
                            sbval.target().create_value_from_address("(as array)", &addr, &array_type)
                        } else {
                            // For other types re-interpret the value itself.
                            let array_type = var_type.array_type(size as u64);
                            let addr = sbval.address().unwrap();
                            sbval.target().create_value_from_address("(as array)", &addr, &array_type)
                        };
                        (var, self.global_format)
                    }
                };

                let handle = self.get_var_handle(None, expression, &var);
                Ok(EvaluateResponseBody {
                    result: self.get_var_value_str(&var, format, handle.is_some()),
                    type_: var.type_name().map(|s| s.to_owned()),
                    variables_reference: handles::to_i64(handle),
                    ..Default::default()
                })
            }
            Err(err) => Err(err),
        }
    }

    // Evaluates expr in the context of frame (or in global context if frame is None)
    // Returns expressions.Value or SBValue on success, SBError on failure.
    fn evaluate_expr_in_frame(
        &self,
        expression: &PreparedExpression,
        frame: Option<&SBFrame>,
    ) -> Result<SBValue, Error> {
        match (expression, self.python.as_ref()) {
            (PreparedExpression::Native(pp_expr), _) => {
                let result = match frame {
                    Some(frame) => frame.evaluate_expression(&pp_expr),
                    None => self.target.evaluate_expression(&pp_expr),
                };
                let error = result.error();
                if error.is_success() {
                    Ok(result)
                } else {
                    Err(error.into())
                }
            }
            (PreparedExpression::Python(pp_expr), Some(python)) => {
                let context = self.context_from_frame(frame);
                match python.evaluate(&pp_expr, false, &context) {
                    Ok(val) => Ok(val),
                    Err(s) => Err(Error::UserError(s)),
                }
            }
            (PreparedExpression::Simple(pp_expr), Some(python)) => {
                let context = self.context_from_frame(frame);
                match python.evaluate(&pp_expr, true, &context) {
                    Ok(val) => Ok(val),
                    Err(s) => Err(Error::UserError(s)),
                }
            }
            _ => Err(Error::UserError("Python expressions are disabled.".into())),
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

    fn handle_set_variable(&mut self, args: SetVariableArguments) -> Result<SetVariableResponseBody, Error> {
        let container_handle = handles::from_i64(args.variables_reference)?;
        let container = self.var_refs.get(container_handle).expect("Invalid variables reference");
        let child = match container {
            Container::SBValue(container) => container.child_member_with_name(&args.name),
            Container::Locals(frame) | Container::Globals(frame) | Container::Statics(frame) => {
                frame.find_variable(&args.name)
            }
            _ => None,
        };
        if let Some(child) = child {
            match child.set_value(&args.value) {
                Ok(()) => {
                    let handle = self.get_var_handle(Some(container_handle), child.name().unwrap_or_default(), &child);
                    let response = SetVariableResponseBody {
                        value: self.get_var_value_str(&child, self.global_format, handle.is_some()),
                        type_: child.type_name().map(|s| s.to_owned()),
                        variables_reference: Some(handles::to_i64(handle)),
                        named_variables: None,
                        indexed_variables: None,
                    };
                    Ok(response)
                }
                Err(err) => Err(Error::UserError(err.to_string())),
            }
        } else {
            Err(Error::UserError("Could not set variable value.".into()))
        }
    }

    fn handle_pause(&mut self, _args: PauseArguments) -> Result<(), Error> {
        let error = self.process.stop();
        if error.is_success() {
            Ok(())
        } else {
            let state = self.process.state();
            if !state.is_running() {
                // Did we lose a 'stopped' event?
                self.notify_process_stopped();
                Ok(())
            } else {
                Err(Error::UserError(error.error_string().into()))
            }
        }
    }

    fn handle_continue(&mut self, _args: ContinueArguments) -> Result<ContinueResponseBody, Error> {
        self.before_resume();
        let error = self.process.resume();
        if error.is_success() {
            Ok(ContinueResponseBody {
                all_threads_continued: Some(true),
            })
        } else {
            if self.process.state().is_running() {
                // Did we lose a 'running' event?
                self.notify_process_running();
                Ok(ContinueResponseBody {
                    all_threads_continued: Some(true),
                })
            } else {
                Err(Error::UserError(error.error_string().into()))
            }
        }
    }

    fn handle_next(&mut self, args: NextArguments) -> Result<(), Error> {
        let thread = match self.process.thread_by_id(args.thread_id as ThreadID) {
            Some(thread) => thread,
            None => {
                error!("Received invalid thread id in step request.");
                return Err(Error::Protocol("Invalid thread id.".into()));
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
                return Err(Error::Protocol("Invalid thread id.".into()));
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
        let thread = self.process.thread_by_id(args.thread_id as ThreadID)?;
        thread.step_out();
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
                return Err(Error::Internal(error));
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
        let (text, cursor_column) = match self.console_mode {
            ConsoleMode::Commands => (&args.text[..], args.column - 1),
            ConsoleMode::Evaluate => {
                if args.text.starts_with('`') {
                    (&args.text[1..], args.column - 2)
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
            None => Err(Error::Protocol("Unexpected goto message.".into())),
            Some(ref goto_args) => {
                let thread_id = args.thread_id as u64;
                match self.process.thread_by_id(thread_id) {
                    None => Err(Error::Protocol("Invalid thread id".into())),
                    Some(thread) => match goto_args.source.source_reference {
                        // Disassembly
                        Some(source_ref) => {
                            let handle = handles::from_i64(source_ref)?;
                            let dasm = self.disassembly.find_by_handle(handle)?;
                            let addr = dasm.address_by_line_num(goto_args.line as u32);
                            let frame = thread.frame_at_index(0).check()?;
                            if frame.set_pc(addr) {
                                self.refresh_client_display(Some(thread_id));
                                Ok(())
                            } else {
                                Err(Error::UserError("Failed to set the instruction pointer.".into()))
                            }
                        }
                        // Normal source file
                        None => {
                            let filespec = SBFileSpec::from(goto_args.source.path.as_ref()?);
                            let result = thread.jump_to_line(&filespec, goto_args.line as u32);
                            if result.is_success() {
                                self.last_goto_request = None;
                                self.refresh_client_display(Some(thread_id));
                                Ok(())
                            } else {
                                Err(Error::UserError(result.error_string().into()))
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
            _ => return Err(Error::Internal("Invalid frameId".into())),
        };
        let thread = frame.thread();
        thread.return_from_frame(&frame); // TODO: ?
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
        let container_handle = handles::from_i64(args.variables_reference?)?;
        let container = self.var_refs.get(container_handle).expect("Invalid variables reference");
        let child = match container {
            Container::SBValue(container) => container.child_member_with_name(&args.name),
            Container::Locals(frame) | Container::Globals(frame) | Container::Statics(frame) => {
                frame.find_variable(&args.name)
            }
            _ => None,
        };
        if let Some(child) = child {
            let addr = child.load_address();
            let size = child.byte_size();
            let data_id = format!("{}/{}", addr, size);
            let desc = child.name().unwrap_or("");
            Ok(DataBreakpointInfoResponseBody {
                data_id: Some(data_id),
                description: format!("{} bytes at {:X} ({})", size, addr, desc),
                ..Default::default()
            })
        } else {
            Ok(DataBreakpointInfoResponseBody {
                data_id: None,
                description: "Variable not found.".into(),
                ..Default::default()
            })
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
            let addr = parts.next()?.parse::<u64>()?;
            let size = parts.next()?.parse::<usize>()?;
            let res = match self.target.watch_address(addr, size, false, true) {
                Ok(wp) => Breakpoint {
                    verified: true,
                    message: Some(format!("{} bytes at {:X} (", size, addr)),
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

        let terminate = match args {
            None => self.process_was_launched,
            Some(args) => match args.terminate_debuggee {
                None => self.process_was_launched,
                Some(terminate) => terminate,
            },
        };
        if let Initialized(ref process) = self.process {
            if terminate {
                process.kill();
            } else {
                process.detach();
            }
        }
        self.shutdown.request_cancellation();
        Ok(())
    }

    fn handle_adapter_settings(&mut self, args: AdapterSettings) -> Result<(), Error> {
        self.update_adapter_settings(&args);
        if self.process.state().is_stopped() {
            self.refresh_client_display(None);
        }
        Ok(())
    }

    fn update_adapter_settings(&mut self, args: &AdapterSettings) {
        self.global_format = match args.display_format {
            None => self.global_format,
            Some(DisplayFormat::Auto) => Format::Default,
            Some(DisplayFormat::Decimal) => Format::Decimal,
            Some(DisplayFormat::Hex) => Format::Hex,
            Some(DisplayFormat::Binary) => Format::Binary,
        };
        self.show_disassembly = args.show_disassembly.unwrap_or(self.show_disassembly);
        self.deref_pointers = args.dereference_pointers.unwrap_or(self.deref_pointers);
        self.suppress_missing_files = args.suppress_missing_source_files.unwrap_or(self.suppress_missing_files);
        self.console_mode = args.console_mode.unwrap_or(self.console_mode);
        if let Some(timeout) = args.evaluation_timeout {
            self.evaluation_timeout = time::Duration::from_millis((timeout * 1000.0) as u64);
        }
        if let Some(ref source_languages) = args.source_languages {
            self.source_languages = source_languages.clone()
        }
        self.default_panic_breakpoint = args.default_panic_breakpoint.unwrap_or(self.default_panic_breakpoint);
        self.default_catch_breakpoint = args.default_catch_breakpoint.unwrap_or(self.default_catch_breakpoint);
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
                    }))
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
            module.filespec().path().display().to_string()
        }
    }

    fn make_module_detail(&self, module: &SBModule) -> Module {
        let mut msg = Module {
            id: serde_json::Value::String(self.module_id(&module)),
            name: module.filespec().filename().display().to_string(),
            path: Some(module.filespec().path().display().to_string()),
            ..Default::default()
        };

        let header_addr = module.object_header_address();
        if header_addr.is_valid() {
            msg.address_range = Some(format!("{:X}", header_addr.load_address(&self.target)));
        }

        let symbols = module.symbol_filespec();
        if symbols.is_valid() {
            msg.symbol_status = Some("Symbols loaded.".into());
            msg.symbol_file_path = Some(module.symbol_filespec().path().display().to_string());
        } else {
            msg.symbol_status = Some("Symbols not found".into())
        }

        msg
    }

    fn handle_breakpoint_event(&mut self, event: &SBBreakpointEvent) {
        let bp = event.breakpoint();
        let event_type = event.event_type();
        let mut breakpoints = self.breakpoints.borrow_mut();

        if event_type.intersects(BreakpointEventType::Added) {
            // Don't notify if we already are tracking this one.
            if let None = breakpoints.breakpoint_infos.get(&bp.id()) {
                let bp_info = BreakpointInfo {
                    id: bp.id(),
                    breakpoint: bp,
                    kind: BreakpointKind::Location,
                    condition: None,
                    log_message: None,
                    hit_condition: None,
                    hit_count: 0,
                };
                self.send_event(EventBody::breakpoint(BreakpointEventBody {
                    reason: "new".into(),
                    breakpoint: self.make_bp_response(&bp_info),
                }));
                breakpoints.breakpoint_infos.insert(bp_info.id, bp_info);
            }
        } else if event_type.intersects(BreakpointEventType::LocationsAdded) {
            if let Some(bp_info) = breakpoints.breakpoint_infos.get_mut(&bp.id()) {
                self.send_event(EventBody::breakpoint(BreakpointEventBody {
                    reason: "changed".into(),
                    breakpoint: self.make_bp_response(bp_info),
                }));
            }
        } else if event_type.intersects(BreakpointEventType::Removed) {
            bp.clear_callback();
            // Send "removed" notification only if we are tracking this breakpoint,
            // otherwise we'd notify VSCode about breakpoints that had been disabled in the UI
            // and cause them to be actually removed.
            if let Some(bp_info) = breakpoints.breakpoint_infos.get_mut(&bp.id()) {
                self.send_event(EventBody::breakpoint(BreakpointEventBody {
                    reason: "removed".into(),
                    breakpoint: Breakpoint {
                        id: Some(bp.id() as i64),
                        ..Default::default()
                    },
                }));
                breakpoints.breakpoint_infos.remove(&bp.id());
            }
        }
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
                    // Check if the file exists.
                    let mapped_path = if self.suppress_missing_files && !path.is_file() {
                        None
                    } else {
                        Some(Rc::new(path))
                    };
                    // Cache the result, so we don't have to probe file system again for the same path.
                    source_map_cache.insert(source_path, mapped_path.clone());
                    mapped_path
                }
            }
        }
    }
}

impl Drop for DebugSession {
    fn drop(&mut self) {
        debug!("### DebugSession::drop()");
    }
}

fn compose_eval_name<'a, 'b, A, B>(prefix: A, suffix: B) -> String
where
    A: Into<Cow<'a, str>>,
    B: Into<Cow<'b, str>>,
{
    let prefix = prefix.into();
    let suffix = suffix.into();
    if prefix.as_ref().is_empty() {
        suffix.into_owned()
    } else if suffix.as_ref().is_empty() {
        prefix.into_owned()
    } else if suffix.as_ref().starts_with("[") {
        (prefix + suffix).into_owned()
    } else {
        (prefix + "." + suffix).into_owned()
    }
}

fn into_string_lossy(cstr: &CStr) -> String {
    cstr.to_string_lossy().into_owned()
}

#[cfg(windows)]
fn put_env(key: &CStr, value: &CStr) {
    use std::os::raw::{c_char, c_int};
    extern "C" {
        fn _putenv_s(key: *const c_char, value: *const c_char) -> c_int;
    }
    unsafe {
        _putenv_s(key.as_ptr(), value.as_ptr());
    }
}

// Async adapter

struct AsyncDebugSession {
    incoming_send: std::sync::mpsc::SyncSender<InputEvent>,
    outgoing_recv: futures::sync::mpsc::Receiver<ProtocolMessage>,
    shutdown_token: CancellationToken,
}

impl Stream for AsyncDebugSession {
    type Item = ProtocolMessage;
    type Error = ();
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.outgoing_recv.poll() {
            Ok(Async::NotReady) if self.shutdown_token.is_cancelled() => {
                error!("Stream::poll after shutdown");
                Ok(Async::Ready(None))
            }
            Ok(r) => Ok(r),
            Err(e) => Err(e),
        }
    }
}

impl Sink for AsyncDebugSession {
    type SinkItem = ProtocolMessage;
    type SinkError = ();
    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        if self.shutdown_token.is_cancelled() {
            Err(())
        } else {
            match self.incoming_send.try_send(InputEvent::ProtocolMessage(item)) {
                Ok(()) => Ok(AsyncSink::Ready),
                Err(err) => match err {
                    std::sync::mpsc::TrySendError::Full(input) | //.
                    std::sync::mpsc::TrySendError::Disconnected(input) => {
                        match input {
                            InputEvent::ProtocolMessage(msg) => Ok(AsyncSink::NotReady(msg)),
                            _ => unreachable!()
                        }
                    }
                },
            }
        }
    }
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}

impl Drop for AsyncDebugSession {
    fn drop(&mut self) {
        debug!("### AsyncDebugSession::drop()");
    }
}
