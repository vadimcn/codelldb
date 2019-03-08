use std;
use std::borrow::Cow;
use std::boxed::FnBox;
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
use crate::handles::{self, Handle, HandleTree};
use crate::must_initialize::{Initialized, MustInitialize, NotInitialized};
use crate::python::{self, PythonInterface, PythonValue};
use crate::source_map::{self, is_same_path, normalize_path};
use crate::terminal::Terminal;
use lldb::*;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdapterParameters {
    evaluation_timeout: Option<u32>,
    suppress_missing_source_files: Option<bool>,
    source_languages: Option<Vec<String>>,
}

type AsyncResponder = FnBox(&mut DebugSession) -> Result<ResponseBody, Error>;

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
    Invoke(Box<FnBox() + Send>),
}

pub struct DebugSession {
    send_message: RefCell<futures::sync::mpsc::Sender<ProtocolMessage>>,
    request_seq: Cell<u32>,
    incoming_send: std::sync::mpsc::SyncSender<InputEvent>,
    shutdown: CancellationSource,
    event_listener: SBListener,
    self_ref: MustInitialize<Weak<Mutex<DebugSession>>>,
    debugger: MustInitialize<SBDebugger>,
    target: MustInitialize<SBTarget>,
    process: MustInitialize<SBProcess>,
    process_launched: bool,
    on_configuration_done: Option<(u32, Box<AsyncResponder>)>,
    python: MustInitialize<Box<PythonInterface>>,
    breakpoints: RefCell<BreakpointsState>,
    var_refs: HandleTree<Container>,
    disassembly: MustInitialize<disassembly::AddressSpace>,
    known_threads: HashSet<ThreadID>,
    source_map_cache: RefCell<HashMap<PathBuf, Option<Rc<PathBuf>>>>,
    loaded_modules: Vec<SBModule>,
    exit_commands: Option<Vec<String>>,
    terminal: Option<Terminal>,
    selected_frame_changed: bool,

    global_format: Format,
    show_disassembly: Option<bool>,
    deref_pointers: bool,
    container_summary: bool,

    default_expr_type: Expressions,
    source_languages: Vec<String>,
    suppress_missing_files: bool,
    evaluation_timeout: time::Duration,
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////

unsafe impl Send for DebugSession {}

impl DebugSession {
    pub fn new(
        parameters: AdapterParameters,
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

        let debug_session = DebugSession {
            send_message: RefCell::new(outgoing_send),
            incoming_send: incoming_send.clone(),
            request_seq: Cell::new(1),
            shutdown: shutdown,
            self_ref: NotInitialized,
            debugger: NotInitialized,
            target: NotInitialized,
            process: NotInitialized,
            process_launched: false,
            event_listener: event_listener,
            on_configuration_done: None,
            python: NotInitialized,
            breakpoints: RefCell::new(BreakpointsState {
                source: HashMap::new(),
                assembly: HashMap::new(),
                function: HashMap::new(),
                breakpoint_infos: HashMap::new(),
            }),
            var_refs: HandleTree::new(),
            disassembly: NotInitialized,
            known_threads: HashSet::new(),
            source_map_cache: RefCell::new(HashMap::new()),
            loaded_modules: Vec::new(),
            exit_commands: None,
            terminal: None,
            selected_frame_changed: false,

            global_format: Format::Default,
            show_disassembly: None,
            deref_pointers: true,
            container_summary: true,

            default_expr_type: Expressions::Simple,
            source_languages: parameters.source_languages.unwrap_or(vec!["cpp".into()]),
            suppress_missing_files: parameters.suppress_missing_source_files.unwrap_or(true),
            evaluation_timeout: time::Duration::from_millis(parameters.evaluation_timeout.unwrap_or(5000).into()),
        };

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
        let result = if let Some(arguments) = request.arguments {
            #[cfg_attr(rustfmt, rustfmt_skip)]
            match arguments {
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
                RequestArguments::source(args) =>
                    self.handle_source(args)
                        .map(|r| ResponseBody::source(r)),
                RequestArguments::completions(args) =>
                    self.handle_completions(args)
                        .map(|r| ResponseBody::completions(r)),
                RequestArguments::disconnect(args) =>
                    self.handle_disconnect(Some(args))
                        .map(|_| ResponseBody::disconnect),
                RequestArguments::displaySettings(args) =>
                    self.handle_display_settings(args)
                        .map(|_| ResponseBody::displaySettings),
                _ => {
                    //error!("No handler for request message: {:?}", request);
                    Err(Error::Internal("Not implemented.".into()))
                }
            }
        } else {
            self.handle_disconnect(None).map(|_| ResponseBody::disconnect)
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
        self.send_message.borrow_mut().try_send(response).map_err(|err| panic!("Could not send response: {}", err));
    }

    fn send_event(&self, event_body: EventBody) {
        let event = ProtocolMessage::Event(Event {
            seq: 0,
            body: event_body,
        });
        self.send_message.borrow_mut().try_send(event).map_err(|err| panic!("Could not send event: {}", err));
    }

    fn send_request(&self, args: RequestArguments) {
        let request = ProtocolMessage::Request(Request {
            seq: self.request_seq.get(),
            arguments: Some(args),
        });
        self.request_seq.set(self.request_seq.get() + 1);
        self.send_message.borrow_mut().try_send(request).map_err(|err| panic!("Could not send request: {}", err));
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
        self.debugger = Initialized(SBDebugger::create(false));
        self.debugger.set_async(true);

        self.event_listener.start_listening_for_event_class(&self.debugger, SBThread::broadcaster_class_name(), !0);

        let send_message = self.send_message.clone();
        let python = PythonInterface::new(
            self.debugger.command_interpreter(),
            Box::new(move |event_body| {
                let event = ProtocolMessage::Event(Event {
                    seq: 0,
                    body: event_body,
                });
                send_message.borrow_mut().try_send(event);
            }),
        )?;
        self.python = Initialized(python);

        let caps = Capabilities {
            supports_configuration_done_request: true,
            supports_evaluate_for_hovers: true,
            supports_function_breakpoints: true,
            supports_conditional_breakpoints: true,
            supports_hit_conditional_breakpoints: true,
            supports_set_variable: true,
            supports_completions_request: true,
            supports_delayed_stack_trace_loading: true,
            support_terminate_debuggee: true,
            supports_log_points: true,
            exception_breakpoint_filters: self.get_exception_filters(&self.source_languages),
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
        &mut self, file_path: &Path, requested_bps: &[SourceBreakpoint],
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
        &mut self, dasm: Rc<disassembly::DisassembledRange>, requested_bps: &[SourceBreakpoint],
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
        &mut self, adapter_data: &disassembly::AdapterData, requested_bps: &[SourceBreakpoint],
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
        &mut self, args: SetFunctionBreakpointsArguments,
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
                default: true,
            });
            filters.push(ExceptionBreakpointsFilter {
                filter: "cpp_catch".into(),
                label: "C++: on catch".into(),
                default: false,
            });
        }
        if source_langs.iter().any(|x| x == "rust") {
            filters.push(ExceptionBreakpointsFilter {
                filter: "rust_panic".into(),
                label: "Rust: on panic".into(),
                default: true,
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
        &self, _process: &SBProcess, thread: &SBThread, location: &SBBreakpointLocation,
        py_condition: &Option<PreparedExpression>, hit_condition: &Option<HitCondition>,
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
            let should_stop = match self.python.evaluate_as_bool(&pycode, is_simple_expr, &context) {
                Ok(val) => val,
                Err(err) => {
                    self.console_error(err.to_string());
                    return true; // Stop on evluation errors, even if there's a log message.
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
        let cb: Box<FnBox() + Send> = Box::new(move || sender.send(f()).unwrap());
        // Casting away cb's lifetime.
        // This is safe, because we are blocking current thread until f() returns.
        let cb: Box<FnBox() + Send + 'static> = unsafe { std::mem::transmute(cb) };
        self_ref.lock().unwrap().incoming_send.send(InputEvent::Invoke(cb)).unwrap();
        receiver.recv().unwrap()
    }

    fn handle_launch(&mut self, args: LaunchRequestArguments) -> Result<Box<AsyncResponder>, Error> {
        if let Some(expressions) = args.expressions {
            self.default_expr_type = expressions;
        }
        if let Some(source_map) = &args.source_map {
            self.init_source_map(source_map.iter().map(|(k, v)| (k, v.as_ref())));
        }
        if let Some(true) = &args.custom {
            self.handle_custom_launch(args)
        } else {
            if let Some(commands) = &args.init_commands {
                self.exec_commands("initCommands", &commands)?;
            }
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
        launch_info.set_launch_flags(launch_info.launch_flags() | LaunchFlag::DisableASLR);

        // Merge environment
        let mut launch_env = HashMap::new();
        for (k, v) in env::vars() {
            launch_env.insert(k, v);
        }
        if let Some(ref env) = args.env {
            for (k, v) in env {
                launch_env.insert(k.clone(), v.clone());
            }
        }
        let launch_env = launch_env.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<String>>();
        launch_info.set_environment_entries(launch_env.iter().map(|s| s.as_ref()), false);

        if let Some(ref ds) = args.display_settings {
            self.update_display_settings(ds);
        }
        if let Some(ref args) = args.args {
            launch_info.set_arguments(args.iter().map(|a| a.as_ref()), false);
        }
        if let Some(ref cwd) = args.cwd {
            launch_info.set_working_directory(&cwd);
        }
        if let Some(stop_on_entry) = args.stop_on_entry {
            if stop_on_entry {
                launch_info.set_launch_flags(launch_info.launch_flags() | LaunchFlag::StopAtEntry);
            }
        }
        self.configure_stdio(&args, &mut launch_info);
        self.target.set_launch_info(&launch_info);

        // Run user commands (which may modify launch info)
        if let Some(ref commands) = args.pre_run_commands {
            self.exec_commands("preRunCommands", commands)?;
        }

        let mut launch_info = self.target.launch_info();
        launch_info.set_listener(&self.event_listener);

        let executable = self.target.executable().path().to_string_lossy().into_owned();
        let command_line = launch_info.arguments().fold(executable, |mut args, a| {
            args.push(' ');
            args.push_str(a);
            args
        });
        self.console_message(format!("Launching: {}", command_line));

        let process = match self.target.launch(&launch_info) {
            Ok(process) => process,
            Err(err) => return Err(Error::UserError(err.error_string().into())),
        };
        self.process = Initialized(process);
        self.process_launched = true;

        if let Some(commands) = args.post_run_commands {
            self.exec_commands("postRunCommands", &commands)?;
        }
        self.exit_commands = args.exit_commands;
        Ok(ResponseBody::launch)
    }

    fn handle_custom_launch(&mut self, args: LaunchRequestArguments) -> Result<Box<AsyncResponder>, Error> {
        if let Some(commands) = &args.target_create_commands.as_ref().or(args.init_commands.as_ref()) {
            self.exec_commands("targetCreateCommands", &commands)?;
        }
        self.target = Initialized(self.debugger.selected_target());
        self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));
        self.send_event(EventBody::initialized);
        Ok(Box::new(move |s: &mut DebugSession| s.complete_custom_launch(args)))
    }

    fn complete_custom_launch(&mut self, args: LaunchRequestArguments) -> Result<ResponseBody, Error> {
        if let Some(commands) = args.process_create_commands.as_ref().or(args.pre_run_commands.as_ref()) {
            self.exec_commands("processCreateCommands", &commands)?;
        }
        self.process = Initialized(self.target.process());
        self.process.broadcaster().add_listener(&self.event_listener, !0);
        self.process_launched = false;
        Ok(ResponseBody::launch)
    }

    fn handle_attach(&mut self, args: AttachRequestArguments) -> Result<Box<AsyncResponder>, Error> {
        if let Some(expressions) = args.expressions {
            self.default_expr_type = expressions;
        }
        if let Some(source_map) = &args.source_map {
            self.init_source_map(source_map.iter().map(|(k, v)| (k, v.as_ref())));
        }
        if args.program.is_none() && args.pid.is_none() {
            return Err(Error::UserError(r#"Either "program" or "pid" is required for attach."#.into()));
        }
        if let Some(commands) = &args.init_commands {
            self.exec_commands("initCommands", &commands)?;
        }
        self.target = Initialized(self.debugger.create_target("", None, None, false)?);
        self.disassembly = Initialized(disassembly::AddressSpace::new(&self.target));
        self.send_event(EventBody::initialized);
        Ok(Box::new(move |s: &mut DebugSession| s.complete_attach(args)))
    }

    fn complete_attach(&mut self, args: AttachRequestArguments) -> Result<ResponseBody, Error> {
        if let Some(ref commands) = args.pre_run_commands {
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
        attach_info.set_wait_for_launch(args.wait_for.unwrap_or(false), true);
        attach_info.set_ignore_existing(false);
        attach_info.set_listener(&self.event_listener);

        let process = match self.target.attach(&attach_info) {
            Ok(process) => process,
            Err(err) => return Err(Error::UserError(err.error_string().into())),
        };
        self.process = Initialized(process);
        self.process_launched = false;

        if !args.stop_on_entry.unwrap_or(false) {
            self.process.resume();
        }
        if let Some(commands) = args.post_run_commands {
            self.exec_commands("postRunCommands", &commands)?;
        }
        self.exit_commands = args.exit_commands;
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
        let terminal_kind = args.terminal.unwrap_or(TerminalKind::Console);

        let tty_name = {
            #[cfg(unix)]
            match terminal_kind {
                TerminalKind::External | TerminalKind::Integrated => {
                    let terminal = Terminal::create(|args| self.run_in_vscode_terminal(terminal_kind.clone(), args))?;
                    let tty_name = terminal.tty_name().to_owned();
                    self.terminal = Some(terminal);
                    Some(tty_name)
                }
                TerminalKind::Console => None,
            }
            #[cfg(windows)]
            {
                let without_console: &[u8] = match terminal_kind {
                    TerminalKind::External => b"false\0",
                    TerminalKind::Integrated | TerminalKind::Console => b"true\0",
                };
                // MSVC's getenv caches environment vars, so setting it via env::set_var() doesn't work.
                put_env(
                    CStr::from_bytes_with_nul(b"LLDB_LAUNCH_INFERIORS_WITHOUT_CONSOLE\0").unwrap(),
                    CStr::from_bytes_with_nul(without_console).unwrap(),
                );
                None
            }
        };

        let mut stdio = match args.stdio {
            Some(ref stdio) => stdio.clone(),
            None => vec![],
        };
        // Pad to at least 3 entries
        while stdio.len() < 3 {
            stdio.push(None)
        }

        for (fd, name) in stdio.iter().enumerate() {
            let (read, write) = match fd {
                0 => (true, false),
                1 => (false, true),
                2 => (false, true),
                _ => (true, true),
            };
            let name = name.as_ref().or(tty_name.as_ref());
            if let Some(name) = name {
                launch_info.add_open_file_action(fd as i32, name, read, write);
            }
        }

        Ok(())
    }

    fn run_in_vscode_terminal(&mut self, terminal_kind: TerminalKind, mut args: Vec<String>) {
        let terminal_kind = match terminal_kind {
            TerminalKind::External => "external",
            TerminalKind::Integrated => {
                args.insert(0, "\n".into());
                "integrated"
            }
            _ => unreachable!(),
        };
        let req_args = RunInTerminalRequestArguments {
            args: args,
            cwd: String::new(),
            env: None,
            kind: Some(terminal_kind.to_owned()),
            title: Some("Debuggee".to_owned()),
        };
        self.send_request(RequestArguments::runInTerminal(req_args));
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
        self.debugger.set_variable("target.source-map", &args);
    }

    fn handle_configuration_done(&mut self) -> Result<(), Error> {
        self.target.broadcaster().add_listener(
            &self.event_listener,
            SBTargetEvent::BroadcastBitBreakpointChanged | SBTargetEvent::BroadcastBitModulesLoaded,
        );
        if let Some((request_seq, responder)) = self.on_configuration_done.take() {
            let result = responder.call_box((self,));

            self.send_response(request_seq, result);

            if self.process.is_initialized() {
                if self.process.state().is_stopped() {
                    self.update_threads();
                    self.send_event(EventBody::stopped(StoppedEventBody {
                        all_threads_stopped: Some(true),
                        thread_id: self.known_threads.iter().next().map(|tid| *tid as i64),
                        reason: "initial".to_owned(),
                        description: None,
                        text: None,
                        preserve_focus_hint: None,
                    }));
                }
            }
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
            Some(v) => v,
            None => {
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
        &mut self, vars_iter: &mut Iterator<Item = SBValue>, container_eval_name: &str,
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
        &mut self, var: &SBValue, container_eval_name: &str, container_handle: Option<Handle>,
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
                if var.is_synthetic() || ptr_type.pointee_type().basic_type() != BasicType::Invalid {
                    if let Some(value_str) = var.summary().map(|s| into_string_lossy(s)) {
                        return value_str;
                    }
                }

                // try dereferencing
                let pointee = var.dereference();
                if !pointee.is_valid() || pointee.data().byte_size() == 0 {
                    if var.value_as_unsigned(0) == 0 {
                        return "<null>".to_owned();
                    } else {
                        return "<invalid address>".to_owned();
                    }
                }
                var = Cow::Owned(pointee);
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
            if self.container_summary {
                self.get_container_summary(var.as_ref())
            } else {
                "{...}".to_owned()
            }
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
            // from frame's thread, instead of the  frame itself.
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
        let mut expression: &str = &args.expression;

        if let Some("repl") = context {
            if !expression.starts_with("?") {
                // LLDB command
                let result = self.execute_command_in_frame(expression, frame.as_ref());
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
            } else {
                expression = &expression[1..]; // drop leading '?'
            }
        }

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
        &self, expression: &PreparedExpression, frame: Option<&SBFrame>,
    ) -> Result<SBValue, Error> {
        match expression {
            PreparedExpression::Native(pp_expr) => {
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
            PreparedExpression::Python(pp_expr) => {
                let context = self.context_from_frame(frame);
                match self.python.evaluate(&pp_expr, false, &context) {
                    Ok(val) => Ok(val),
                    Err(s) => Err(Error::UserError(s)),
                }
            }
            PreparedExpression::Simple(pp_expr) => {
                let context = self.context_from_frame(frame);
                match self.python.evaluate(&pp_expr, true, &context) {
                    Ok(val) => Ok(val),
                    Err(s) => Err(Error::UserError(s)),
                }
            }
        }
    }

    fn execute_command_in_frame(&self, command: &str, frame: Option<&SBFrame>) -> SBCommandReturnObject {
        let context = self.context_from_frame(frame);
        let mut result = SBCommandReturnObject::new();
        let interp = self.debugger.command_interpreter();
        let ok = interp.handle_command_with_context(command, &context, &mut result, false);
        debug!("{} -> {:?}, {:?}", command, ok, result);
        // TODO: multiline
        result
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
                        variables_reference: handles::to_i64(handle),
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
            thread.step_over();
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
            thread.step_into();
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

    fn handle_source(&mut self, args: SourceArguments) -> Result<SourceResponseBody, Error> {
        let handle = handles::from_i64(args.source_reference)?;
        let dasm = self.disassembly.find_by_handle(handle).unwrap();
        Ok(SourceResponseBody {
            content: dasm.get_source_text(),
            mime_type: Some("text/x-lldb.disassembly".to_owned()),
        })
    }

    fn handle_completions(&mut self, args: CompletionsArguments) -> Result<CompletionsResponseBody, Error> {
        let interpreter = self.debugger.command_interpreter();
        let targets = match interpreter.handle_completions(&args.text, (args.column - 1) as u32, None) {
            None => vec![],
            Some((common_continuation, completions)) => {
                // LLDB completions usually include some tail of the string being completed, without telling us what that prefix is.
                // For example, completing "set show tar" might return ["target.arg0", "target.auto-apply-fixits", ...].

                // Compute cursor position inside args.text in as byte offset.
                let cursor_index = args
                    .text
                    .char_indices()
                    .skip((args.column - 1) as usize)
                    .next()
                    .map(|p| p.0)
                    .unwrap_or(args.text.len());
                // Take a slice up to the cursor, split it on whitespaces, then get the last part.
                // This is the (likely) prefix of completions returned by LLDB.
                let prefix = &args.text[..cursor_index].split_whitespace().next_back().unwrap_or_default();
                let prefix_len = prefix.chars().count();
                let extended_prefix = format!("{}{}", prefix, common_continuation);

                let mut targets = vec![];
                for completion in completions {
                    // Check if we guessed the prefix correctly
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

    fn handle_disconnect(&mut self, args: Option<DisconnectArguments>) -> Result<(), Error> {
        if let Some(commands) = &self.exit_commands {
            self.exec_commands("exitCommands", &commands)?;
        }
        let terminate = match args {
            None => self.process_launched,
            Some(args) => match args.terminate_debuggee {
                None => self.process_launched,
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

    fn handle_display_settings(&mut self, args: DisplaySettingsArguments) -> Result<(), Error> {
        self.update_display_settings(&args);
        self.refresh_client_display();
        Ok(())
    }

    fn update_display_settings(&mut self, args: &DisplaySettingsArguments) {
        self.global_format = match args.display_format {
            None => self.global_format,
            Some(DisplayFormat::Auto) => Format::Default,
            Some(DisplayFormat::Decimal) => Format::Decimal,
            Some(DisplayFormat::Hex) => Format::Hex,
            Some(DisplayFormat::Binary) => Format::Binary,
        };
        self.show_disassembly = match args.show_disassembly {
            None => self.show_disassembly,
            Some(ShowDisassembly::Auto) => None,
            Some(ShowDisassembly::Always) => Some(true),
            Some(ShowDisassembly::Never) => Some(false),
        };
        self.deref_pointers = match args.dereference_pointers {
            None => self.deref_pointers,
            Some(v) => v,
        };
        self.container_summary = match args.container_summary {
            None => self.container_summary,
            Some(v) => v,
        };
        // Show current settings
        let show_disasm = match self.show_disassembly {
            None => "auto",
            Some(true) => "always",
            Some(false) => "never",
        };
        let msg = format!("Display settings: variable format={}, show disassembly={}, numeric pointer values={}, container summaries={}.",
            format!("{:?}", self.global_format).to_lowercase(),
            show_disasm,
            if self.deref_pointers { "on" } else { "off" },
            if self.container_summary { "on" } else { "off" }
        );
        self.console_message(msg);
    }

    // Fake target start/stop to force VSCode to refresh UI state.
    fn refresh_client_display(&mut self) {
        let thread_id = self.process.selected_thread().thread_id();
        self.send_event(EventBody::continued(ContinuedEventBody {
            thread_id: thread_id as i64,
            all_threads_continued: Some(true),
        }));
        self.send_event(EventBody::stopped(StoppedEventBody {
            thread_id: Some(thread_id as i64),
            //preserve_focus_hint: Some(true),
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
            let (read_stream, category): (&for<'r> Fn(&mut [u8]) -> usize, &str) =
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
        }))
    }

    fn notify_process_stopped(&mut self) {
        self.update_threads();
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

        self.python.modules_loaded(&mut self.loaded_modules.iter());
        self.loaded_modules.clear();
    }

    // Notify VSCode about target threads that started or exited since the last stop.
    fn update_threads(&mut self) {
        let threads = self.process.threads().map(|t| t.thread_id()).collect::<HashSet<_>>();
        let started = threads.difference(&self.known_threads).cloned().collect::<Vec<_>>();
        let exited = self.known_threads.difference(&threads).cloned().collect::<Vec<_>>();
        for tid in exited {
            self.send_event(EventBody::thread(ThreadEventBody {
                thread_id: tid as i64,
                reason: "exited".to_owned(),
            }));
        }
        for tid in started {
            self.send_event(EventBody::thread(ThreadEventBody {
                thread_id: tid as i64,
                reason: "started".to_owned(),
            }));
        }
        self.known_threads = threads;
    }

    fn handle_target_event(&mut self, event: &SBTargetEvent) {
        let flags = event.as_event().flags();
        if flags & SBTargetEvent::BroadcastBitModulesLoaded != 0 {
            // Running scripts during target execution seems to trigger a bug in LLDB,
            // so we defer loaded module notification till the next stop.
            for module in event.modules() {
                let mut message = format!("Module loaded: {}.", module.filespec().path().display());
                let symbols = module.symbol_filespec();
                if symbols.is_valid() {
                    message.push_str(" Symbols loaded.");
                }
                self.console_message(message);

                self.loaded_modules.push(module);
            }
        } else if flags & SBTargetEvent::BroadcastBitSymbolsLoaded != 0 {
            for module in event.modules() {
                self.console_message(format!("Symbols loaded: {}", module.symbol_filespec().path().display()));
            }
        } else if flags & SBTargetEvent::BroadcastBitModulesUnloaded != 0 {
            for module in event.modules() {
                let message = format!("Module Unloaded: {}", module.filespec().path().display());
                self.console_message(message);
            }
        }
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

    fn map_filespec_to_local(&self, filespec: &SBFileSpec) -> Option<Rc<PathBuf>> {
        if !filespec.is_valid() {
            return None;
        } else {
            let source_path = filespec.path();
            let mut source_map_cache = self.source_map_cache.borrow_mut();
            match source_map_cache.get(&source_path) {
                Some(mapped_path) => mapped_path.clone(),
                None => {
                    let path = filespec.path();
                    let mapped_path = if self.suppress_missing_files && !path.is_file() {
                        None
                    } else {
                        Some(Rc::new(path))
                    };
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
