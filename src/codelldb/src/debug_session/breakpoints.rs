use crate::dap_session::DAPSession;
use crate::debug_session::DebugSession;
use crate::disassembly;
use crate::expressions::{self, HitCondition, PreparedExpression};
use crate::fsutil::normalize_path;
use crate::handles::Handle;
use crate::prelude::*;
use crate::python::{EvalContext, PyObject, PythonSession};

use std::collections::HashMap;
use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use adapter_protocol::*;
use lldb::*;

#[derive(Debug)]
enum BreakpointKind {
    Source,
    Disassembly,
    Instruction,
    Function,
    Exception(String), // Exception breakpoint with the given name.
}

#[derive(Debug)]
struct BreakpointInfo {
    id: BreakpointID,
    breakpoint: SBBreakpoint,
    kind: BreakpointKind,
    hit_condition: Option<HitCondition>,
    hit_count: u32,
    exclusions: Vec<String>,
}

pub(super) struct Breakpoints {
    breakpoint_infos: HashMap<BreakpointID, Arc<Mutex<BreakpointInfo>>>,
    source: HashMap<PathBuf, HashMap<i64, BreakpointID>>,
    assembly: HashMap<Handle, HashMap<i64, BreakpointID>>,
    legacy_assembly: HashMap<disassembly::AdapterData, HashMap<i64, BreakpointID>>,
    instruction: HashMap<Address, BreakpointID>,
    function: HashMap<String, BreakpointID>,
}

impl Breakpoints {
    pub(super) fn new() -> Self {
        Breakpoints {
            breakpoint_infos: HashMap::new(),
            source: HashMap::new(),
            assembly: HashMap::new(),
            legacy_assembly: HashMap::new(),
            instruction: HashMap::new(),
            function: HashMap::new(),
        }
    }

    fn forget_id(&mut self, id: BreakpointID) {
        for lines in self.source.values_mut() {
            lines.retain(|_, stored_id| *stored_id != id);
        }
        for lines in self.assembly.values_mut() {
            lines.retain(|_, stored_id| *stored_id != id);
        }
        for lines in self.legacy_assembly.values_mut() {
            lines.retain(|_, stored_id| *stored_id != id);
        }
        self.instruction.retain(|_, stored_id| *stored_id != id);
        self.function.retain(|_, stored_id| *stored_id != id);
    }
}

fn remove_tracked_breakpoint(
    target: &SBTarget,
    breakpoint_infos: &mut HashMap<BreakpointID, Arc<Mutex<BreakpointInfo>>>,
    id: BreakpointID,
) {
    if let Some(info) = breakpoint_infos.remove(&id) {
        let breakpoint = info.lock().unwrap().breakpoint.clone();
        breakpoint.clear_callback();
    }
    target.breakpoint_delete(id);
}

fn replace_breakpoint_index<K: Eq + Hash>(
    target: &SBTarget,
    breakpoint_infos: &mut HashMap<BreakpointID, Arc<Mutex<BreakpointInfo>>>,
    existing: &mut HashMap<K, BreakpointID>,
    replacement: HashMap<K, BreakpointID>,
) {
    for (key, id) in existing.iter() {
        if replacement.get(key) != Some(id) {
            remove_tracked_breakpoint(target, breakpoint_infos, *id);
        }
    }
    *existing = replacement;
}

fn breakpoint_should_stop(
    info: &mut BreakpointInfo,
    thread: &SBThread,
    py_condition: Option<&(PyObject, EvalContext)>,
    python: Option<&PythonSession>,
    dap_session: &DAPSession,
) -> bool {
    if !info.exclusions.is_empty()
        && thread
            .frames()
            .filter_map(|frame| frame.pc_address().symbol())
            .any(|symbol| info.exclusions.iter().any(|exclusion| exclusion == symbol.name()))
    {
        return false;
    }

    if let Some((pycode, eval_context)) = py_condition {
        let frame = thread.frame_at_index(0);
        let exec_context = SBExecutionContext::from_frame(&frame);
        // TODO: pass bpno
        let should_stop = match python {
            Some(python) => match python.evaluate_as_bool(pycode, &exec_context, *eval_context) {
                Ok(value) => value,
                Err(err) => {
                    let event = EventBody::output(OutputEventBody {
                        output: format!("Could not evaluate breakpoint condition:\n{}\n", err),
                        category: Some("stderr".into()),
                        ..Default::default()
                    });
                    log_errors!(dap_session.try_send_event(event));
                    return true; // Stop on evaluation errors.
                }
            },
            None => return true,
        };
        if !should_stop {
            return false;
        }
    }

    // LLDB counts callbacks even when they return false, but does not count hits rejected by a native condition.
    info.hit_count += 1;
    info.hit_condition.as_ref().map_or(true, |condition| condition.matches(info.hit_count))
}

const CPP_THROW: &str = "cpp_throw";
const CPP_CATCH: &str = "cpp_catch";
const RUST_PANIC: &str = "rust_panic";
const SWIFT_THROW: &str = "swift_throw";

impl DebugSession {
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
        let Breakpoints {
            ref mut source,
            ref mut breakpoint_infos,
            ..
        } = *self.breakpoints.borrow_mut();

        let file_path_norm = normalize_path(file_path);
        let existing_bps = source.entry(file_path.into()).or_default();
        let mut new_bps = HashMap::new();
        let mut result = vec![];
        for req in requested_bps {
            // Find an existing breakpoint or create a new one
            let bp = match existing_bps.get(&req.line).and_then(|bp_id| self.target.find_breakpoint_by_id(*bp_id)) {
                Some(bp) => bp,
                None => {
                    let location = match self.breakpoint_mode {
                        BreakpointMode::Path => file_path_norm.as_os_str(),
                        BreakpointMode::File => file_path_norm.file_name().ok_or("Missing file name")?,
                    };
                    self.target.breakpoint_create_by_location(
                        Path::new(location),
                        req.line as u32,
                        req.column.map(|c| c as u32),
                    )
                }
            };

            let bp_info = self.register_breakpoint(
                bp,
                BreakpointKind::Source,
                req.condition.as_deref(),
                req.hit_condition.as_deref(),
            );
            let (bp_id, num_locations, mut breakpoint) = {
                let bp_info = bp_info.lock().unwrap();
                (
                    bp_info.id,
                    bp_info.breakpoint.num_locations(),
                    self.make_bp_response(&bp_info),
                )
            };
            // If the breakpoint can't be resolved, try BreakpointMode::File mode and report findings.
            if num_locations == 0 && matches!(self.breakpoint_mode, BreakpointMode::Path) {
                if let Some((path, line)) = self.get_breakpoint_hint(&file_path_norm, req.line, req.column) {
                    let message = format!(
                        "Breakpoint at {}:{} could not be resolved, but a valid location was found at {}:{}",
                        file_path_norm.display(),
                        req.line,
                        path.display(),
                        line
                    );
                    self.console_message(&message);
                    breakpoint.message = Some(message);
                }
            }
            result.push(breakpoint);
            new_bps.insert(req.line, bp_id);
            breakpoint_infos.insert(bp_id, bp_info);
        }

        replace_breakpoint_index(&self.target, breakpoint_infos, existing_bps, new_bps);
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
        let Breakpoints {
            ref mut assembly,
            ref mut breakpoint_infos,
            ..
        } = *self.breakpoints.borrow_mut();
        let existing_bps = assembly.entry(dasm.handle()).or_default();
        let mut new_bps = HashMap::new();
        let mut result = vec![];
        for req in requested_bps {
            let laddress = dasm.address_by_line_num(req.line as u32);

            // Find an existing breakpoint or create a new one
            let bp = match existing_bps.get(&req.line).and_then(|bp_id| self.target.find_breakpoint_by_id(*bp_id)) {
                Some(bp) => bp,
                None => self.target.breakpoint_create_by_load_address(laddress),
            };

            let bp_info = self.register_breakpoint(
                bp,
                BreakpointKind::Disassembly,
                req.condition.as_deref(),
                req.hit_condition.as_deref(),
            );
            let (bp_id, response) = {
                let bp_info = bp_info.lock().unwrap();
                (bp_info.id, self.make_bp_response(&bp_info))
            };
            result.push(response);
            new_bps.insert(req.line, bp_id);
            breakpoint_infos.insert(bp_id, bp_info);
        }
        replace_breakpoint_index(&self.target, breakpoint_infos, existing_bps, new_bps);
        Ok(result)
    }

    fn set_new_dasm_breakpoints(
        &mut self,
        adapter_data: &disassembly::AdapterData,
        requested_bps: &[SourceBreakpoint],
    ) -> Result<Vec<Breakpoint>, Error> {
        let Breakpoints {
            ref mut legacy_assembly,
            ref mut breakpoint_infos,
            ..
        } = *self.breakpoints.borrow_mut();
        let existing_bps = legacy_assembly.entry(adapter_data.clone()).or_default();
        let mut new_bps = HashMap::new();
        let mut result = vec![];
        let line_addresses = disassembly::DisassembledRange::lines_from_adapter_data(adapter_data);
        for req in requested_bps {
            if req.line >= 0 && req.line < line_addresses.len() as i64 {
                let address = line_addresses[req.line as usize] as Address;
                let bp = match existing_bps.get(&req.line).and_then(|id| self.target.find_breakpoint_by_id(*id)) {
                    Some(bp) => bp,
                    None => self.target.breakpoint_create_by_load_address(address),
                };
                let bp_info = self.register_breakpoint(
                    bp,
                    BreakpointKind::Disassembly,
                    req.condition.as_deref(),
                    req.hit_condition.as_deref(),
                );
                let (bp_id, response) = {
                    let bp_info = bp_info.lock().unwrap();
                    (bp_info.id, self.make_bp_response(&bp_info))
                };
                result.push(response);
                new_bps.insert(req.line, bp_id);
                breakpoint_infos.insert(bp_id, bp_info);
            } else {
                result.push(Default::default());
            }
        }
        replace_breakpoint_index(&self.target, breakpoint_infos, existing_bps, new_bps);
        Ok(result)
    }

    pub(super) fn handle_set_instruction_breakpoints(
        &mut self,
        args: SetInstructionBreakpointsArguments,
    ) -> Result<SetInstructionBreakpointsResponseBody, Error> {
        let Breakpoints {
            ref mut instruction,
            ref mut breakpoint_infos,
            ..
        } = *self.breakpoints.borrow_mut();

        let mut new_bps = HashMap::new();
        let mut result = vec![];
        for req in args.breakpoints {
            // Find an existing breakpoint or create a new one
            let base_addr = parse_int::parse::<u64>(&req.instruction_reference)?;
            let address = base_addr.wrapping_add(req.offset.unwrap_or(0) as u64);

            let bp = match instruction.get(&address).and_then(|bp_id| self.target.find_breakpoint_by_id(*bp_id)) {
                Some(bp) => bp,
                None => self.target.breakpoint_create_by_load_address(address),
            };

            let bp_info = self.register_breakpoint(
                bp,
                BreakpointKind::Instruction,
                req.condition.as_deref(),
                req.hit_condition.as_deref(),
            );
            let (bp_id, response) = {
                let bp_info = bp_info.lock().unwrap();
                (bp_info.id, self.make_bp_response(&bp_info))
            };

            result.push(response);
            new_bps.insert(address, bp_id);
            breakpoint_infos.insert(bp_id, bp_info);
        }
        replace_breakpoint_index(&self.target, breakpoint_infos, instruction, new_bps);

        Ok(SetInstructionBreakpointsResponseBody { breakpoints: result })
    }

    pub(super) fn handle_set_function_breakpoints(
        &mut self,
        args: SetFunctionBreakpointsArguments,
    ) -> Result<SetBreakpointsResponseBody, Error> {
        let Breakpoints {
            ref mut function,
            ref mut breakpoint_infos,
            ..
        } = *self.breakpoints.borrow_mut();
        let mut new_bps = HashMap::new();
        let mut result = vec![];
        for req in args.breakpoints {
            // Find an existing breakpoint or create a new one
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

            let bp_info = self.register_breakpoint(
                bp,
                BreakpointKind::Function,
                req.condition.as_deref(),
                req.hit_condition.as_deref(),
            );
            let (bp_id, response) = {
                let bp_info = bp_info.lock().unwrap();
                (bp_info.id, self.make_bp_response(&bp_info))
            };

            result.push(response);
            new_bps.insert(req.name, bp_id);
            breakpoint_infos.insert(bp_id, bp_info);
        }
        replace_breakpoint_index(&self.target, breakpoint_infos, function, new_bps);

        Ok(SetBreakpointsResponseBody { breakpoints: result })
    }

    pub(super) fn handle_set_exception_breakpoints(
        &mut self,
        args: SetExceptionBreakpointsArguments,
    ) -> Result<SetExceptionBreakpointsResponseBody, Error> {
        let mut breakpoints = self.breakpoints.borrow_mut();
        let old_ids: Vec<_> = breakpoints
            .breakpoint_infos
            .iter()
            .filter_map(|(&id, info)| matches!(info.lock().unwrap().kind, BreakpointKind::Exception(_)).then_some(id))
            .collect();
        for id in old_ids {
            remove_tracked_breakpoint(&self.target, &mut breakpoints.breakpoint_infos, id);
        }

        let mut result = vec![];
        for exc_name in &args.filters {
            if let Some(bp) = self.set_exception_breakpoint(exc_name) {
                let bp_info = self.register_breakpoint(bp, BreakpointKind::Exception(exc_name.into()), None, None);
                let (bp_id, response) = {
                    let bp_info = bp_info.lock().unwrap();
                    (bp_info.id, self.make_bp_response(&bp_info))
                };
                result.push(response);
                breakpoints.breakpoint_infos.insert(bp_id, bp_info);
            }
        }
        for filter in &args.filter_options {
            if let Some(bp) = self.set_exception_breakpoint(&filter.filter_id) {
                let bp_info = self.register_breakpoint(
                    bp,
                    BreakpointKind::Exception(filter.filter_id.clone()),
                    filter.condition.as_deref(),
                    None,
                );
                let (bp_id, response) = {
                    let bp_info = bp_info.lock().unwrap();
                    (bp_info.id, self.make_bp_response(&bp_info))
                };
                result.push(response);
                breakpoints.breakpoint_infos.insert(bp_id, bp_info);
            }
        }
        Ok(SetExceptionBreakpointsResponseBody { breakpoints: result })
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

    fn register_breakpoint(
        &self,
        bp: SBBreakpoint,
        kind: BreakpointKind,
        condition: Option<&str>,
        hit_condition: Option<&str>,
    ) -> Arc<Mutex<BreakpointInfo>> {
        fn empty2none(s: Option<&str>) -> Option<&str> {
            s.map(str::trim).filter(|s| !s.is_empty())
        }

        let py_condition = self.prepare_breakpoint_condition(&bp, empty2none(condition));
        let bp_info = BreakpointInfo {
            id: bp.id(),
            breakpoint: bp,
            kind,
            hit_condition: empty2none(hit_condition).and_then(|expr| match expressions::parse_hit_condition(expr) {
                Ok(cond) => Some(cond),
                Err(_) => {
                    self.console_error(format!("Invalid hit condition: {}", expr));
                    None
                }
            }),
            hit_count: 0,
            exclusions: Vec::new(),
        };

        let python = self.python.clone();
        let dap_session = self.dap_session.clone();
        let bp_info = Arc::new(Mutex::new(bp_info));
        let weak_bp_info = Arc::downgrade(&bp_info);

        bp_info.lock().unwrap().breakpoint.set_callback(move |_process, thread, location| {
            debug!("Callback for breakpoint location {:?}", location);
            let Some(bp_info) = weak_bp_info.upgrade() else {
                return true;
            };
            let mut bp_info = bp_info.lock().unwrap();
            breakpoint_should_stop(
                &mut bp_info,
                thread,
                py_condition.as_ref(),
                python.as_deref(),
                &dap_session,
            )
        });

        bp_info
    }

    fn prepare_breakpoint_condition(
        &self,
        bp: &SBBreakpoint,
        condition: Option<&str>,
    ) -> Option<(PyObject, EvalContext)> {
        let condition = condition?;
        let prepared = match expressions::prepare(condition, self.default_expr_type) {
            Ok(prepared) => prepared,
            Err(err) => {
                self.console_error(format!("Could not parse breakpoint condition:\n{}", err));
                return None;
            }
        };
        let (expr, eval_context) = match prepared {
            PreparedExpression::Native(expr) => {
                bp.set_condition(&expr);
                return None;
            }
            PreparedExpression::Simple(expr) => (expr, EvalContext::SimpleExpression),
            PreparedExpression::Python(expr) => (expr, EvalContext::PythonExpression),
        };
        let python = self.python.as_ref()?;
        match python.compile_code(&expr, "<breakpoint condition>") {
            Ok(pycode) => Some((pycode, eval_context)),
            Err(err) => {
                self.console_error(format!("Could not parse breakpoint condition:\n{}", err));
                None
            }
        }
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

        match &info.kind {
            BreakpointKind::Source | BreakpointKind::Function => {
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
            BreakpointKind::Disassembly => {
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
            BreakpointKind::Exception(_) => {}
        }
        response
    }

    pub(super) fn handle_breakpoint_event(&mut self, event: &SBBreakpointEvent) {
        let bp = event.breakpoint();
        let event_type = event.event_type();
        let mut breakpoints = self.breakpoints.borrow_mut();

        if event_type.intersects(BreakpointEventType::Removed) {
            bp.clear_callback();
            if breakpoints.breakpoint_infos.remove(&bp.id()).is_some() {
                breakpoints.forget_id(bp.id());
                self.send_event(EventBody::breakpoint(BreakpointEventBody {
                    reason: "removed".into(),
                    breakpoint: Breakpoint {
                        id: Some(bp.id() as i64),
                        ..Default::default()
                    },
                }));
            }
        } else if event_type.intersects(BreakpointEventType::LocationsAdded | BreakpointEventType::LocationsResolved) {
            if let Some(info) = breakpoints.breakpoint_infos.get(&bp.id()) {
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
        let Some(bp_info) = breakpoints.breakpoint_infos.get(&bp_id) else {
            bail!(blame_user(str_error("Could not locate symbol for this stack frame.")));
        };
        let mut bp_info = bp_info.lock().unwrap();
        bp_info.exclusions.push(symbol.clone());

        let (site_id, label) = if let BreakpointKind::Exception(exc_name) = &bp_info.kind {
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
        for bp_info in breakpoints.breakpoint_infos.values() {
            let mut bp_info = bp_info.lock().unwrap();
            bp_info.exclusions.clear();
        }
        for exclusion in args.exclusions {
            match exclusion.site_id {
                Either::First(bp_id) => {
                    let bp_id = bp_id as BreakpointID;
                    if let Some(bp_info) = breakpoints.breakpoint_infos.get(&bp_id) {
                        let mut bp_info = bp_info.lock().unwrap();
                        bp_info.exclusions.push(exclusion.symbol);
                    }
                }
                Either::Second(exc_name) => {
                    for bp_info in breakpoints.breakpoint_infos.values() {
                        let mut bp_info = bp_info.lock().unwrap();
                        if let BreakpointKind::Exception(exc_name2) = &bp_info.kind {
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
