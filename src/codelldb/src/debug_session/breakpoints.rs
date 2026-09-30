use crate::debug_session::log_template::LogTemplate;
use crate::debug_session::value_formatter::ValueFormatter;
use crate::debug_session::DebugSession;
use crate::disassembly;
use crate::expressions::{HitCondition, PreparedExpression};
use crate::fsutil::normalize_path;
use crate::handles::Handle;
use crate::prelude::*;

use std::collections::HashMap;
use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use adapter_protocol::*;
use lldb::*;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum BreakpointKind {
    Source(PathBuf),
    Disassembly(Handle),
    LegacyDisassembly(disassembly::AdapterData),
    Instruction,
    Function,
    Exception,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum BreakpointLocation {
    Source(i64, Option<i64>),
    Line(i64),
    Address(Address),
    Name(String),
}

impl From<(i64, Option<i64>)> for BreakpointLocation {
    fn from((line, column): (i64, Option<i64>)) -> Self {
        Self::Source(line, column)
    }
}

impl From<i64> for BreakpointLocation {
    fn from(line: i64) -> Self {
        Self::Line(line)
    }
}

impl From<Address> for BreakpointLocation {
    fn from(address: Address) -> Self {
        Self::Address(address)
    }
}

impl From<String> for BreakpointLocation {
    fn from(name: String) -> Self {
        Self::Name(name)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct BreakpointKey {
    kind: BreakpointKind,
    location: BreakpointLocation,
}

#[derive(Debug)]
struct BreakpointInfo {
    id: BreakpointID,
    breakpoint: SBBreakpoint,
    key: BreakpointKey,
    hit_condition: Option<HitCondition>,
    hit_count: u32,
    exclusions: Vec<String>,
}

struct BreakpointRequest<'a, K> {
    key: K,
    condition: Option<&'a str>,
    hit_condition: Option<&'a str>,
    log_message: Option<&'a str>,
}

pub(super) struct Breakpoints {
    by_key: HashMap<BreakpointKey, Arc<Mutex<BreakpointInfo>>>,
    by_id: HashMap<BreakpointID, Arc<Mutex<BreakpointInfo>>>,
}

impl Breakpoints {
    pub(super) fn new() -> Self {
        Breakpoints {
            by_key: HashMap::new(),
            by_id: HashMap::new(),
        }
    }

    fn insert(&mut self, info: Arc<Mutex<BreakpointInfo>>) {
        let info_guard = info.lock().unwrap();
        let id = info_guard.id;
        let key = info_guard.key.clone();
        drop(info_guard);
        self.by_key.insert(key, info.clone());
        self.by_id.insert(id, info);
    }
}

fn remove_tracked_breakpoint(target: &SBTarget, breakpoints: &mut Breakpoints, id: BreakpointID) {
    if let Some(info) = breakpoints.by_id.remove(&id) {
        let info = info.lock().unwrap();
        breakpoints.by_key.remove(&info.key);
        let breakpoint = info.breakpoint.clone();
        drop(info);
        breakpoint.clear_callback();
    }
    target.breakpoint_delete(id);
}

const CPP_THROW: &str = "cpp_throw";
const CPP_CATCH: &str = "cpp_catch";
const RUST_PANIC: &str = "rust_panic";
const SWIFT_THROW: &str = "swift_throw";

impl DebugSession {
    fn update_breakpoints<'a, K: Clone + Into<BreakpointLocation>>(
        &self,
        breakpoints: &mut Breakpoints,
        kind: BreakpointKind,
        requests: impl IntoIterator<Item = BreakpointRequest<'a, K>>,
        mut create: impl FnMut(&K) -> Option<SBBreakpoint>,
    ) -> Vec<Breakpoint> {
        let old_keys: Vec<_> = breakpoints.by_key.keys().filter(|key| key.kind == kind).cloned().collect();
        let mut old: HashMap<_, _> = old_keys
            .into_iter()
            .map(|key| {
                let info = breakpoints.by_key.remove(&key).unwrap();
                (key, info)
            })
            .collect();
        let mut replacement = HashMap::new();
        let mut responses = Vec::new();

        for request in requests {
            let BreakpointRequest {
                key,
                condition,
                hit_condition,
                log_message,
            } = request;
            let breakpoint_key = BreakpointKey {
                kind: kind.clone(),
                location: key.clone().into(),
            };

            // A repeated key uses the breakpoint from the previous request entry.
            let info = match replacement.remove(&breakpoint_key).or_else(|| old.remove(&breakpoint_key)) {
                Some(info) => {
                    let id = info.lock().unwrap().id;
                    if self.target.find_breakpoint_by_id(id).is_some() {
                        Some(info)
                    } else {
                        remove_tracked_breakpoint(&self.target, breakpoints, id);
                        None
                    }
                }
                None => None,
            };

            let info = if let Some(info) = info {
                info
            } else if let Some(bp) = create(&key) {
                Arc::new(Mutex::new(BreakpointInfo {
                    id: bp.id(),
                    breakpoint: bp,
                    key: breakpoint_key.clone(),
                    hit_condition: None,
                    hit_count: 0,
                    exclusions: Vec::new(),
                }))
            } else {
                responses.push(Breakpoint::default());
                continue;
            };

            self.configure_breakpoint(&info, condition, hit_condition, log_message);
            responses.push(self.make_bp_response(&info.lock().unwrap()));
            replacement.insert(breakpoint_key, info);
        }

        for info in old.into_values() {
            let id = info.lock().unwrap().id;
            remove_tracked_breakpoint(&self.target, breakpoints, id);
        }
        for info in replacement.into_values() {
            breakpoints.insert(info);
        }
        responses
    }

    pub(super) fn handle_set_breakpoints(
        &mut self,
        args: SetBreakpointsArguments,
    ) -> Result<SetBreakpointsResponseBody, Error> {
        // Decide whether this is a real source file or a disassembled range:
        // if it has a `source_reference` attribute, it's a disassembled range - we never generate references for real sources;
        // if it has an `adapter_data` attribute, it's a disassembled range from a previous debug session;
        // otherwise, it's a real source file (and we expect it to have a valid `path` attribute).
        let dasm = args
            .source
            .source_reference
            .and_then(|source_ref| self.disasm_ranges.find_by_handle(source_ref));

        let breakpoints = match (dasm, args.source.adapter_data, args.source.path.as_ref()) {
            (Some(dasm), _, _) => self.set_dasm_breakpoints(dasm, &args.breakpoints),
            (None, Some(adapter_data), _) => self.set_new_dasm_breakpoints(
                &serde_json::from_value::<disassembly::AdapterData>(adapter_data)?,
                &args.breakpoints,
            ),
            (None, None, Some(path)) => self.set_source_breakpoints(Path::new(path), &args.breakpoints),
            _ => bail!("Unexpected"),
        }?;
        Ok(SetBreakpointsResponseBody { breakpoints })
    }

    fn set_source_breakpoints(
        &mut self,
        file_path: &Path,
        requested_bps: &[SourceBreakpoint],
    ) -> Result<Vec<Breakpoint>, Error> {
        let file_path_norm = normalize_path(file_path);
        let location = match self.breakpoint_mode {
            BreakpointMode::Path => file_path_norm.as_os_str(),
            BreakpointMode::File => file_path_norm.file_name().ok_or("Missing file name")?,
        };
        let kind = BreakpointKind::Source(file_path.into());
        let mut breakpoints = self.breakpoints.borrow_mut();
        let mut result = self.update_breakpoints(
            &mut breakpoints,
            kind,
            requested_bps.iter().map(|req| BreakpointRequest {
                key: (req.line, req.column),
                condition: req.condition.as_deref(),
                hit_condition: req.hit_condition.as_deref(),
                log_message: req.log_message.as_deref(),
            }),
            |&(line, column)| {
                Some(self.target.breakpoint_create_by_location(
                    Path::new(location),
                    line as u32,
                    column.map(|column| column as u32),
                ))
            },
        );
        drop(breakpoints);

        // If a source breakpoint can't be resolved, suggest a likely location in another file.
        if matches!(self.breakpoint_mode, BreakpointMode::Path) {
            for (req, response) in requested_bps.iter().zip(&mut result) {
                if response.id.is_some() && !response.verified {
                    if let Some((path, line)) = self.get_breakpoint_hint(&file_path_norm, req.line, req.column) {
                        let message = format!(
                            "Breakpoint at {}:{} could not be resolved, but a valid location was found at {}:{}",
                            file_path_norm.display(),
                            req.line,
                            path.display(),
                            line
                        );
                        self.console_message(&message);
                        response.message = Some(message);
                    }
                }
            }
        }
        Ok(result)
    }

    // Try to find likely location for a breakpoint in a different file.
    fn get_breakpoint_hint(&self, filepath: &Path, line: i64, column: Option<i64>) -> Option<(PathBuf, u32)> {
        let Some(filename) = filepath.file_name() else {
            return None;
        };
        let line = line as u32;
        let column = column.map(|c| c as u32);

        let bp = self.target.breakpoint_create_by_location(Path::new(filename), line, column);

        // If we end up with multiple locations, select the one with the longest path match.
        let mut best_count = 0;
        let mut best_path = None;
        for loc in bp.locations() {
            if let Some(le) = loc.address().line_entry() {
                if le.line() == line {
                    let path = le.file_spec().path();
                    let count = path
                        .components()
                        .rev()
                        .zip(filepath.components().rev())
                        .take_while(|(a, b)| a == b)
                        .count();
                    if count > best_count {
                        best_count = count;
                        best_path = Some(path);
                    }
                }
            }
        }
        self.target.breakpoint_delete(bp.id());

        if let Some(best_path) = best_path {
            Some((best_path, line))
        } else {
            None
        }
    }

    fn set_dasm_breakpoints(
        &mut self,
        dasm: Rc<disassembly::DisassembledRange>,
        requested_bps: &[SourceBreakpoint],
    ) -> Result<Vec<Breakpoint>, Error> {
        let kind = BreakpointKind::Disassembly(dasm.handle());
        let mut breakpoints = self.breakpoints.borrow_mut();
        Ok(self.update_breakpoints(
            &mut breakpoints,
            kind,
            requested_bps.iter().map(|req| BreakpointRequest {
                key: req.line,
                condition: req.condition.as_deref(),
                hit_condition: req.hit_condition.as_deref(),
                log_message: req.log_message.as_deref(),
            }),
            |&line| {
                let address = dasm.address_by_line_num(line as u32);
                Some(self.target.breakpoint_create_by_load_address(address))
            },
        ))
    }

    fn set_new_dasm_breakpoints(
        &mut self,
        adapter_data: &disassembly::AdapterData,
        requested_bps: &[SourceBreakpoint],
    ) -> Result<Vec<Breakpoint>, Error> {
        let kind = BreakpointKind::LegacyDisassembly(adapter_data.clone());
        let line_addresses = disassembly::DisassembledRange::lines_from_adapter_data(adapter_data);
        let mut breakpoints = self.breakpoints.borrow_mut();
        Ok(self.update_breakpoints(
            &mut breakpoints,
            kind,
            requested_bps.iter().map(|req| BreakpointRequest {
                key: req.line,
                condition: req.condition.as_deref(),
                hit_condition: req.hit_condition.as_deref(),
                log_message: req.log_message.as_deref(),
            }),
            |&line| {
                if line < 0 {
                    return None;
                }
                let address = *line_addresses.get(line as usize)? as Address;
                Some(self.target.breakpoint_create_by_load_address(address))
            },
        ))
    }

    pub(super) fn handle_set_instruction_breakpoints(
        &mut self,
        args: SetInstructionBreakpointsArguments,
    ) -> Result<SetInstructionBreakpointsResponseBody, Error> {
        let requested = args
            .breakpoints
            .iter()
            .map(|req| {
                let base_addr = parse_int::parse::<u64>(&req.instruction_reference)?;
                let address = base_addr.wrapping_add(req.offset.unwrap_or(0) as u64);
                Ok(BreakpointRequest {
                    key: address,
                    condition: req.condition.as_deref(),
                    hit_condition: req.hit_condition.as_deref(),
                    log_message: None,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let mut breakpoints = self.breakpoints.borrow_mut();
        let breakpoints = self.update_breakpoints(
            &mut breakpoints,
            BreakpointKind::Instruction,
            requested,
            |&address| Some(self.target.breakpoint_create_by_load_address(address)),
        );
        Ok(SetInstructionBreakpointsResponseBody { breakpoints })
    }

    pub(super) fn handle_set_function_breakpoints(
        &mut self,
        args: SetFunctionBreakpointsArguments,
    ) -> Result<SetBreakpointsResponseBody, Error> {
        let mut breakpoints = self.breakpoints.borrow_mut();
        let breakpoints = self.update_breakpoints(
            &mut breakpoints,
            BreakpointKind::Function,
            args.breakpoints.iter().map(|req| BreakpointRequest {
                key: req.name.clone(),
                condition: req.condition.as_deref(),
                hit_condition: req.hit_condition.as_deref(),
                log_message: None,
            }),
            |name| {
                Some(if let Some(regex) = name.strip_prefix("/re ") {
                    self.target.breakpoint_create_by_regex(regex)
                } else {
                    self.target.breakpoint_create_by_name(name)
                })
            },
        );
        Ok(SetBreakpointsResponseBody { breakpoints })
    }

    pub(super) fn handle_set_exception_breakpoints(
        &mut self,
        args: SetExceptionBreakpointsArguments,
    ) -> Result<SetExceptionBreakpointsResponseBody, Error> {
        let requested = args
            .filters
            .iter()
            .map(|filter_id| BreakpointRequest {
                key: filter_id.clone(),
                condition: None,
                hit_condition: None,
                log_message: None,
            })
            .chain(args.filter_options.iter().map(|filter| BreakpointRequest {
                key: filter.filter_id.clone(),
                condition: filter.condition.as_deref(),
                hit_condition: None,
                log_message: None,
            }));
        let mut breakpoints = self.breakpoints.borrow_mut();
        let breakpoints = self.update_breakpoints(
            &mut breakpoints,
            BreakpointKind::Exception,
            requested,
            |filter_id| self.set_exception_breakpoint(filter_id),
        );
        Ok(SetExceptionBreakpointsResponseBody { breakpoints })
    }

    pub(super) fn get_exception_filters() -> &'static [ExceptionBreakpointsFilter] {
        lazy_static::lazy_static! {
            static ref FILTERS: [ExceptionBreakpointsFilter; 4] = [
                ExceptionBreakpointsFilter {
                    filter: CPP_THROW.into(),
                    label: "C++: on throw".into(),
                    default: Some(true),
                    supports_condition: Some(true),
                    ..Default::default()
                },
                ExceptionBreakpointsFilter {
                    filter: CPP_CATCH.into(),
                    label: "C++: on catch".into(),
                    default: Some(false),
                    supports_condition: Some(true),
                    ..Default::default()
                },
                ExceptionBreakpointsFilter {
                    filter: RUST_PANIC.into(),
                    label: "Rust: on panic".into(),
                    default: Some(true),
                    supports_condition: Some(true),
                    ..Default::default()
                },
                ExceptionBreakpointsFilter {
                    filter: SWIFT_THROW.into(),
                    label: "Swift: on throw".into(),
                    default: Some(false),
                    supports_condition: Some(true),
                    ..Default::default()
                }
            ];
        }
        &*FILTERS
    }

    fn set_exception_breakpoint(&self, exc_name: &str) -> Option<SBBreakpoint> {
        match exc_name {
            CPP_THROW => {
                let bp = self.target.breakpoint_create_for_exception(LanguageType::C_plus_plus, false, true);
                bp.add_name("cpp_exception");
                Some(bp)
            }
            CPP_CATCH => {
                let bp = self.target.breakpoint_create_for_exception(LanguageType::C_plus_plus, true, false);
                bp.add_name("cpp_exception");
                Some(bp)
            }
            RUST_PANIC => {
                let bp = self.target.breakpoint_create_by_regex("rust_panic$");
                bp.add_name("rust_panic");
                Some(bp)
            }
            SWIFT_THROW => {
                let bp = self.target.breakpoint_create_for_exception(LanguageType::Swift, false, true);
                bp.add_name("swift_exception");
                Some(bp)
            }
            _ => None,
        }
    }

    fn configure_breakpoint(
        &self,
        bp_info: &Arc<Mutex<BreakpointInfo>>,
        condition: Option<&str>,
        hit_condition: Option<&str>,
        log_message: Option<&str>,
    ) {
        fn empty2none(s: Option<&str>) -> Option<&str> {
            s.map(str::trim).filter(|s| !s.is_empty())
        }

        let bp = bp_info.lock().unwrap().breakpoint.clone();
        bp.set_condition("");
        let py_condition = empty2none(condition).and_then(|condition| {
            let prepared = match PreparedExpression::prepare(
                condition,
                self.default_expr_type,
                self.python.as_ref(),
                "<breakpoint condition>",
            ) {
                Ok(prepared) => prepared,
                Err(err) => {
                    self.console_error(format!("Could not parse breakpoint condition:\n{}", err));
                    return None;
                }
            };
            if let Some(expr) = prepared.native_source() {
                bp.set_condition(expr);
                None
            } else {
                Some(prepared)
            }
        });

        let hit_condition = empty2none(hit_condition).and_then(|expr| match HitCondition::parse(expr) {
            Ok(hc) => Some(hc),
            Err(_) => {
                self.console_error(format!("Invalid hit condition: {}", expr));
                None
            }
        });

        let logpoint = empty2none(log_message).and_then(|message| {
            match LogTemplate::prepare(message, self.default_expr_type, self.python.as_ref()) {
                Ok(template) => Some((template, ValueFormatter::from_session(self, None))),
                Err(error) => {
                    self.console_error(error);
                    None
                }
            }
        });

        bp_info.lock().unwrap().hit_condition = hit_condition;

        let is_logpoint = logpoint.is_some();
        let dap_session = self.dap_session.clone();
        let console_message = move |category: &str, mut output: String| {
            output.push('\n');
            log_errors!(dap_session.try_send_event(EventBody::output(OutputEventBody {
                output: output,
                category: Some(category.into()),
                ..Default::default()
            })));
        };

        let weak_bp_info = Arc::downgrade(bp_info);

        bp.set_callback(move |_process, thread, location| {
            debug!("Callback for breakpoint location {:?}", location);

            let bp_info = weak_bp_info.upgrade().expect("BreakpointInfo");
            let mut bp_info = bp_info.lock().unwrap();

            // Evaluate Python condition
            if let Some(condition) = &py_condition {
                let frame = thread.frame_at_index(0);
                // TODO: pass bpno
                let result = condition.evaluate_as_bool_in_frame(&frame).map_err(|err| err.to_string());
                match result {
                    Ok(true) => {}
                    Ok(false) => return false,
                    Err(err) => {
                        drop(bp_info);
                        console_message("stderr", format!("Could not evaluate breakpoint condition:\n{err}"));
                        return !is_logpoint;
                    }
                }
            }

            // LLDB does not count hits rejected by a native condition, but does count callbacks returning false,
            // so we maintain our own hit count for consistency.
            bp_info.hit_count += 1;

            // Evaluate hit_condition
            if let Some(ref hit_condition) = bp_info.hit_condition {
                if !hit_condition.matches(bp_info.hit_count) {
                    return false;
                }
            }

            // Handle caller exclusions
            if !bp_info.exclusions.is_empty() {
                for frame in thread.frames() {
                    if let Some(symbol) = frame.pc_address().symbol() {
                        let sym_name = symbol.name();
                        if bp_info.exclusions.iter().any(|exclusion| exclusion == sym_name) {
                            return false;
                        }
                    }
                }
            }

            // Handle logpoints
            if let Some((template, value_formatter)) = &logpoint {
                match template.render(&thread.frame_at_index(0), value_formatter) {
                    Ok(output) => console_message("console", output),
                    Err(error) => console_message("stderr", error),
                }
                return false;
            }

            true
        });
    }

    // Generates a DAP Breakpoint from a registered breakpoint.
    fn make_bp_response(&self, info: &BreakpointInfo) -> Breakpoint {
        let bp = &info.breakpoint;
        let mut response = Breakpoint {
            id: Some(info.id as i64),
            verified: bp.num_locations() > 0,
            message: Some(format!("Resolved locations: {}", bp.num_resolved_locations())),
            ..Default::default()
        };
        if !response.verified {
            return response;
        }

        match &info.key.kind {
            BreakpointKind::Source(_) | BreakpointKind::Function => {
                if let Some(line_entry) = bp.location_at_index(0).address().line_entry() {
                    let file_path = line_entry.file_spec().path();
                    response.source = Some(Source {
                        name: Some(file_path.file_name().unwrap().to_string_lossy().into_owned()),
                        path: Some(file_path.as_os_str().to_string_lossy().into_owned()),
                        ..Default::default()
                    });
                    response.line = Some(line_entry.line() as i64);
                }
            }
            BreakpointKind::Disassembly(_) | BreakpointKind::LegacyDisassembly(_) => {
                let address = bp.location_at_index(0).address().load_address(&self.target);
                if let Ok(dasm) = self.disasm_ranges.from_address(address) {
                    response.line = Some(dasm.line_num_by_address(address) as i64);
                    response.source = Some(Source {
                        name: Some(dasm.source_name().to_owned()),
                        path: Some(dasm.source_name().to_owned()),
                        source_reference: Some(dasm.handle()),
                        adapter_data: Some(serde_json::to_value(dasm.adapter_data()).unwrap()),
                        ..Default::default()
                    });
                } else {
                    response.verified = false;
                }
            }
            BreakpointKind::Instruction => {
                let address = bp.location_at_index(0).address().load_address(&self.target);
                response.instruction_reference = Some(format!("0x{:X}", address));
            }
            BreakpointKind::Exception => {}
        }
        response
    }

    pub(super) fn handle_breakpoint_event(&mut self, event: &SBBreakpointEvent) {
        let bp = event.breakpoint();
        let event_type = event.event_type();
        let mut breakpoints = self.breakpoints.borrow_mut();

        if event_type.intersects(BreakpointEventType::Removed) {
            bp.clear_callback();
            if let Some(info) = breakpoints.by_id.remove(&bp.id()) {
                let key = info.lock().unwrap().key.clone();
                breakpoints.by_key.remove(&key);
                self.send_event(EventBody::breakpoint(BreakpointEventBody {
                    reason: "removed".into(),
                    breakpoint: Breakpoint {
                        id: Some(bp.id() as i64),
                        ..Default::default()
                    },
                }));
            }
        } else if event_type.intersects(BreakpointEventType::LocationsAdded | BreakpointEventType::LocationsResolved) {
            if let Some(info) = breakpoints.by_id.get(&bp.id()) {
                let response = self.make_bp_response(&info.lock().unwrap());
                self.send_event(EventBody::breakpoint(BreakpointEventBody {
                    reason: "changed".into(),
                    breakpoint: response,
                }));
            }
        } else if event_type.intersects(BreakpointEventType::Added) {
            info!("Breakpoint added {}", bp.id());
        }
    }

    pub(super) fn handle_exclude_caller(&mut self, args: ExcludeCallerRequest) -> Result<ExcludeCallerResponse, Error> {
        let thread = self.target.process().selected_thread();
        if thread.stop_reason() != StopReason::Breakpoint {
            bail!(blame_user(str_error("Must be stopped on a breakpoint.")));
        }
        let bp_id = thread.stop_reason_data_at_index(0) as BreakpointID;
        let Some(symbol) = self.symbol_from_frame(args.thread_id as ThreadID, args.frame_index) else {
            bail!(blame_user(str_error("Could not locate symbol for this stack frame.")));
        };
        let breakpoints = self.breakpoints.borrow();
        let Some(bp_info) = breakpoints.by_id.get(&bp_id) else {
            bail!(blame_user(str_error("Could not locate symbol for this stack frame.")));
        };
        let mut bp_info = bp_info.lock().unwrap();
        bp_info.exclusions.push(symbol.clone());

        let (site_id, label) = if let (BreakpointKind::Exception, BreakpointLocation::Name(exc_name)) =
            (&bp_info.key.kind, &bp_info.key.location)
        {
            let filter = DebugSession::get_exception_filters().iter().find(|f| f.filter == *exc_name);
            (Either::Second(exc_name.clone()), filter.unwrap().label.clone())
        } else {
            let label = if let Some(le) = bp_info.breakpoint.location_at_index(0).address().line_entry() {
                format!("{}:{}", le.file_spec().filename().display().to_string(), le.line())
            } else {
                String::new()
            };
            (Either::First(bp_id as i64), label)
        };
        Ok(ExcludeCallerResponse {
            exclusion: ExcludedCaller { site_id, symbol },
            label,
        })
    }

    fn symbol_from_frame(&self, thread_id: ThreadID, frame_index: u32) -> Option<String> {
        let thread = self.target.process().thread_by_id(thread_id)?;
        thread.frame_at_index(frame_index).pc_address().symbol().map(|s| s.name().to_owned())
    }

    pub(super) fn handle_set_excluded_callers(&mut self, args: SetExcludedCallersRequest) -> Result<(), Error> {
        let breakpoints = self.breakpoints.borrow();
        for bp_info in breakpoints.by_id.values() {
            let mut bp_info = bp_info.lock().unwrap();
            bp_info.exclusions.clear();
        }
        for exclusion in args.exclusions {
            match exclusion.site_id {
                Either::First(bp_id) => {
                    let bp_id = bp_id as BreakpointID;
                    if let Some(bp_info) = breakpoints.by_id.get(&bp_id) {
                        let mut bp_info = bp_info.lock().unwrap();
                        bp_info.exclusions.push(exclusion.symbol);
                    }
                }
                Either::Second(exc_name) => {
                    for bp_info in breakpoints.by_id.values() {
                        let mut bp_info = bp_info.lock().unwrap();
                        if let (BreakpointKind::Exception, BreakpointLocation::Name(exc_name2)) =
                            (&bp_info.key.kind, &bp_info.key.location)
                        {
                            if &exc_name == exc_name2 {
                                bp_info.exclusions.push(exclusion.symbol.clone());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
