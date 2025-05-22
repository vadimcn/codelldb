use crate::debug_session::DebugSession;
use crate::disassembly;
use crate::expressions::{self, HitCondition, PreparedExpression};
use crate::fsutil::normalize_path;
use crate::handles::{self, Handle};
use crate::prelude::*;
use crate::python::{EvalContext, PyObject};

use std::collections::HashMap;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use adapter_protocol::*;
use lldb::*;

#[derive(Debug, Clone)]
enum BreakpointKind {
    Source,
    Disassembly,
    Instruction,
    Function,
    Exception(String), // Exception breakpoint with the given name.
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
    exclusions: Vec<String>,
}

pub(super) struct Breakpoints {
    breakpoint_infos: HashMap<BreakpointID, BreakpointInfo>,
    source: HashMap<PathBuf, HashMap<i64, BreakpointID>>,
    assembly: HashMap<Handle, HashMap<i64, BreakpointID>>,
    instruction: HashMap<Address, BreakpointID>,
    function: HashMap<String, BreakpointID>,
}

impl Breakpoints {
    pub(super) fn new() -> Self {
        Breakpoints {
            breakpoint_infos: HashMap::new(),
            source: HashMap::new(),
            assembly: HashMap::new(),
            instruction: HashMap::new(),
            function: HashMap::new(),
        }
    }
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
        let requested_bps = args.breakpoints.as_ref().ok_or("breakpoints")?;
        // Decide whether this is a real source file or a disassembled range:
        // if it has a `source_reference` attribute, it's a disassembled range - we never generate references for real sources;
        // if it has an `adapter_data` attribute, it's a disassembled range from a previous debug session;
        // otherwise, it's a real source file (and we expect it to have a valid `path` attribute).
        let dasm = args
            .source
            .source_reference
            .map(|source_ref| handles::from_i64(source_ref).unwrap())
            .and_then(|source_ref| self.disasm_ranges.find_by_handle(source_ref));

        let breakpoints = match (dasm, args.source.adapter_data, args.source.path.as_ref()) {
            (Some(dasm), _, _) => self.set_dasm_breakpoints(dasm, requested_bps),
            (None, Some(adapter_data), _) => self.set_new_dasm_breakpoints(
                &serde_json::from_value::<disassembly::AdapterData>(adapter_data)?,
                requested_bps,
            ),
            (None, None, Some(path)) => self.set_source_breakpoints(Path::new(path), requested_bps),
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

            let bp_info = self.make_bp_info(
                bp,
                BreakpointKind::Source,
                req.condition.as_deref(),
                req.log_message.as_deref(),
                req.hit_condition.as_deref(),
            );
            self.init_bp_actions(&bp_info);
            let mut breakpoint = self.make_bp_response(&bp_info, false);
            // If the breakpoint can't be resolved, try BreakpointMode::File mode and report findings.
            if bp_info.breakpoint.num_locations() == 0 && matches!(self.breakpoint_mode, BreakpointMode::Path) {
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
            new_bps.insert(req.line, bp_info.id);
            breakpoint_infos.insert(bp_info.id, bp_info);
        }

        for (line, bp_id) in existing_bps.iter() {
            if !new_bps.contains_key(line) {
                self.target.breakpoint_delete(*bp_id);
                breakpoint_infos.remove(bp_id);
            }
        }
        drop(mem::replace(existing_bps, new_bps));
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

            let bp_info = self.make_bp_info(
                bp,
                BreakpointKind::Disassembly,
                req.condition.as_deref(),
                req.log_message.as_deref(),
                req.hit_condition.as_deref(),
            );
            self.init_bp_actions(&bp_info);
            result.push(self.make_bp_response(&bp_info, false));
            new_bps.insert(req.line, bp_info.id);
            breakpoint_infos.insert(bp_info.id, bp_info);
        }
        for (line, bp_id) in existing_bps.iter() {
            if !new_bps.contains_key(line) {
                self.target.breakpoint_delete(*bp_id);
            }
        }
        drop(mem::replace(existing_bps, new_bps));
        Ok(result)
    }

    fn set_new_dasm_breakpoints(
        &mut self,
        adapter_data: &disassembly::AdapterData,
        requested_bps: &[SourceBreakpoint],
    ) -> Result<Vec<Breakpoint>, Error> {
        let mut result = vec![];
        let line_addresses = disassembly::DisassembledRange::lines_from_adapter_data(adapter_data);
        for req in requested_bps {
            if req.line >= 0 && req.line < line_addresses.len() as i64 {
                let address = line_addresses[req.line as usize] as Address;
                let bp = self.target.breakpoint_create_by_load_address(address);
                let bp_info = self.make_bp_info(
                    bp,
                    BreakpointKind::Disassembly,
                    req.condition.as_deref(),
                    req.log_message.as_deref(),
                    req.hit_condition.as_deref(),
                );
                self.init_bp_actions(&bp_info);
                result.push(self.make_bp_response(&bp_info, false));
                self.breakpoints.get_mut().breakpoint_infos.insert(bp_info.id, bp_info);
            } else {
                result.push(Default::default());
            }
        }
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

            let bp_info = self.make_bp_info(
                bp,
                BreakpointKind::Instruction,
                req.condition.as_deref(),
                None,
                req.hit_condition.as_deref(),
            );
            self.init_bp_actions(&bp_info);

            result.push(self.make_bp_response(&bp_info, false));
            new_bps.insert(address, bp_info.id);
            breakpoint_infos.insert(bp_info.id, bp_info);
        }
        for (name, bp_id) in instruction.iter() {
            if !new_bps.contains_key(name) {
                self.target.breakpoint_delete(*bp_id);
            }
        }
        drop(mem::replace(instruction, new_bps));

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

            let bp_info = self.make_bp_info(
                bp,
                BreakpointKind::Function,
                req.condition.as_deref(),
                None,
                req.hit_condition.as_deref(),
            );
            self.init_bp_actions(&bp_info);

            result.push(self.make_bp_response(&bp_info, false));
            new_bps.insert(req.name, bp_info.id);
            breakpoint_infos.insert(bp_info.id, bp_info);
        }
        for (name, bp_id) in function.iter() {
            if !new_bps.contains_key(name) {
                self.target.breakpoint_delete(*bp_id);
            }
        }
        drop(mem::replace(function, new_bps));

        Ok(SetBreakpointsResponseBody { breakpoints: result })
    }

    pub(super) fn handle_set_exception_breakpoints(
        &mut self,
        args: SetExceptionBreakpointsArguments,
    ) -> Result<(), Error> {
        let mut breakpoints = self.breakpoints.borrow_mut();
        breakpoints.breakpoint_infos.retain(|_id, bp_info| {
            if let BreakpointKind::Exception(_) = bp_info.kind {
                self.target.breakpoint_delete(bp_info.id);
                false
            } else {
                true
            }
        });

        for exc_name in &args.filters {
            if let Some(bp) = self.set_exception_breakpoint(exc_name) {
                let bp_info = self.make_bp_info(bp, BreakpointKind::Exception(exc_name.into()), None, None, None);
                self.init_bp_actions(&bp_info);
                breakpoints.breakpoint_infos.insert(bp_info.id, bp_info);
            }
        }
        if let Some(filter_options) = &args.filter_options {
            for filter in filter_options {
                if let Some(bp) = self.set_exception_breakpoint(&filter.filter_id) {
                    let bp_info = self.make_bp_info(
                        bp,
                        BreakpointKind::Exception(filter.filter_id.clone()),
                        filter.condition.as_deref(),
                        None,
                        None,
                    );
                    self.init_bp_actions(&bp_info);
                    breakpoints.breakpoint_infos.insert(bp_info.id, bp_info);
                }
            }
        }
        Ok(())
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
                let bp = self.target.breakpoint_create_by_name("rust_panic");
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

    fn make_bp_info(
        &self,
        bp: SBBreakpoint,
        kind: BreakpointKind,
        condition: Option<&str>,
        log_message: Option<&str>,
        hit_condition: Option<&str>,
    ) -> BreakpointInfo {
        fn empty2none(s: Option<&str>) -> Option<&str> {
            s.map(str::trim).filter(|s| !s.is_empty())
        }

        BreakpointInfo {
            id: bp.id(),
            breakpoint: bp,
            kind: kind,
            condition: empty2none(condition).map(Into::into),
            log_message: empty2none(log_message).map(Into::into),
            hit_condition: empty2none(hit_condition).and_then(|expr| match expressions::parse_hit_condition(expr) {
                Ok(cond) => Some(cond),
                Err(_) => {
                    self.console_error(format!("Invalid hit condition: {}", expr));
                    None
                }
            }),
            hit_count: 0,
            exclusions: Vec::new(),
        }
    }

    // Generates debug_protocol::Breakpoint message from a BreakpointInfo
    fn make_bp_response(&self, bp_info: &BreakpointInfo, include_source: bool) -> Breakpoint {
        let message = Some(format!(
            "Resolved locations: {}",
            bp_info.breakpoint.num_resolved_locations()
        ));

        if bp_info.breakpoint.num_locations() == 0 {
            Breakpoint {
                id: Some(bp_info.id as i64),
                verified: false,
                message,
                ..Default::default()
            }
        } else {
            match &bp_info.kind {
                BreakpointKind::Source => {
                    let address = bp_info.breakpoint.location_at_index(0).address();
                    if let Some(le) = address.line_entry() {
                        let source = if include_source {
                            let file_path = le.file_spec().path();
                            Some(Source {
                                name: Some(file_path.file_name().unwrap().to_string_lossy().into_owned()),
                                path: Some(file_path.as_os_str().to_string_lossy().into_owned()),
                                ..Default::default()
                            })
                        } else {
                            None
                        };

                        Breakpoint {
                            id: Some(bp_info.id as i64),
                            source: source,
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
                BreakpointKind::Disassembly => {
                    let address = bp_info.breakpoint.location_at_index(0).address();
                    let laddress = address.load_address(&self.target);
                    if let Ok(dasm) = self.disasm_ranges.from_address(laddress) {
                        let adapter_data = Some(serde_json::to_value(dasm.adapter_data()).unwrap());
                        Breakpoint {
                            id: Some(bp_info.id as i64),
                            verified: true,
                            line: Some(dasm.line_num_by_address(laddress) as i64),
                            source: Some(Source {
                                name: Some(dasm.source_name().to_owned()),
                                path: Some(dasm.source_name().to_owned()),
                                source_reference: Some(handles::to_i64(Some(dasm.handle()))),
                                adapter_data: adapter_data,
                                ..Default::default()
                            }),
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
                BreakpointKind::Instruction => {
                    let address = bp_info.breakpoint.location_at_index(0).address();
                    let laddress = address.load_address(&self.target);
                    Breakpoint {
                        id: Some(bp_info.id as i64),
                        verified: true,
                        instruction_reference: Some(format!("0x{:X}", laddress)),
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
                BreakpointKind::Exception(_) => Breakpoint {
                    id: Some(bp_info.id as i64),
                    verified: bp_info.breakpoint.num_locations() > 0,
                    message,
                    ..Default::default()
                },
            }
        }
    }

    // Propagate breakpoint options from BreakpointInfo into the associated SBBreakpoint.
    fn init_bp_actions(&self, bp_info: &BreakpointInfo) {
        // Determine type of the break condition expression.
        let py_condition: Option<(PyObject, EvalContext)> = if let Some(ref condition) = bp_info.condition {
            match expressions::prepare(condition, self.default_expr_type) {
                Ok(pp_expr) => match &pp_expr {
                    // if native, use that directly,
                    PreparedExpression::Native(expr) => {
                        bp_info.breakpoint.set_condition(&expr);
                        None
                    }
                    // otherwise, we'll need to evaluate it ourselves in the breakpoint callback.
                    PreparedExpression::Simple(expr) | //.
                    PreparedExpression::Python(expr) => {
                        if let Some(python) = &self.python {
                            match python.compile_code(&expr, "<breakpoint condition>") {
                                Ok(pycode) => {
                                    let eval_context = if matches!(pp_expr, PreparedExpression::Simple(_)) {
                                        EvalContext::SimpleExpression
                                    } else {
                                        EvalContext::PythonExpression
                                    };
                                    Some((pycode, eval_context))
                                }
                                Err(err) => {
                                    self.console_error(format!("Could not parse breakpoint condition:\n{}", err));
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    }
                },
                Err(err) => {
                    self.console_error(format!("Could not parse breakpoint condition:\n{}", err));
                    None
                }
            }
        } else {
            None
        };

        let shared_session = self.self_ref.clone();
        let rt = tokio::runtime::Handle::current();
        bp_info.breakpoint.set_callback(move |process, thread, location| {
            debug!("Callback for breakpoint location {:?}", location);
            rt.block_on(shared_session.map(|s| s.on_breakpoint_hit(process, thread, location, &py_condition)))
        });
    }

    fn on_breakpoint_hit(
        &self,
        _process: &SBProcess,
        thread: &SBThread,
        location: &SBBreakpointLocation,
        py_condition: &Option<(PyObject, EvalContext)>,
    ) -> bool {
        let mut breakpoints = self.breakpoints.borrow_mut();
        let bp_info = breakpoints.breakpoint_infos.get_mut(&location.breakpoint().id()).unwrap();

        if !bp_info.exclusions.is_empty() {
            let symbols_on_stack: Vec<String> = thread
                .frames()
                .filter_map(|f| {
                    if let Some(symbol) = f.pc_address().symbol() {
                        Some(symbol.name().to_owned())
                    } else {
                        None
                    }
                })
                .collect();

            for exclusion in &bp_info.exclusions {
                if symbols_on_stack.iter().any(|s| s == exclusion) {
                    return false;
                }
            }
        }

        if let Some((pycode, eval_context)) = py_condition {
            let frame = thread.frame_at_index(0);
            let exec_context = self.context_from_frame(Some(&frame));
            // TODO: pass bpno
            let should_stop = match &self.python {
                Some(python) => match python.evaluate_as_bool(&pycode, &exec_context, *eval_context) {
                    Ok(val) => val,
                    Err(err) => {
                        self.console_error(format!("Could not evaluate breakpoint condition:\n{}", err));
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
        // however it does count ones where the callback was invoked, even if it had returned false.
        bp_info.hit_count += 1;

        if let Some(hit_condition) = &bp_info.hit_condition {
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
            F: Fn(&str) -> Result<String, Error>,
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
            let (pp_expr, format_spec) = expressions::prepare_with_format(expr, self.default_expr_type).unwrap();
            let sbval = self.evaluate_expr_in_frame(&pp_expr, Some(frame))?;
            let sbval = self.apply_format_spec(sbval, &format_spec)?;
            let str_val = self.get_var_summary(&sbval, false);
            Ok(str_val)
        })
    }

    pub(super) fn handle_breakpoint_event(&mut self, event: &SBBreakpointEvent) {
        let bp = event.breakpoint();
        let event_type = event.event_type();
        let mut breakpoints = self.breakpoints.borrow_mut();

        if event_type.intersects(BreakpointEventType::Added) {
            // Don't notify client if we are already tracking this one.
            // Also, don't notify for transient breakpoints.
            if breakpoints.breakpoint_infos.contains_key(&bp.id()) || !bp.is_valid() {
                return;
            }
            let bp_info = self.make_bp_info(bp, BreakpointKind::Source, None, None, None);
            self.send_event(EventBody::breakpoint(BreakpointEventBody {
                reason: "new".into(),
                breakpoint: self.make_bp_response(&bp_info, true),
            }));
            breakpoints.breakpoint_infos.insert(bp_info.id, bp_info);
        } else if event_type.intersects(BreakpointEventType::LocationsAdded | BreakpointEventType::LocationsResolved) {
            if let Some(bp_info) = breakpoints.breakpoint_infos.get(&bp.id()) {
                self.send_event(EventBody::breakpoint(BreakpointEventBody {
                    reason: "changed".into(),
                    breakpoint: self.make_bp_response(bp_info, false),
                }));
            }
        } else if event_type.intersects(BreakpointEventType::Removed) {
            bp.clear_callback();
            // Send "removed" notification only if we are tracking this breakpoint,
            // otherwise we'd notify VSCode about breakpoints that had been disabled in the UI
            // and cause them to be removed from VSCode UI altogether.
            if breakpoints.breakpoint_infos.contains_key(&bp.id()) {
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

    pub(super) fn handle_exclude_caller(&mut self, args: ExcludeCallerRequest) -> Result<ExcludeCallerResponse, Error> {
        let thread = self.target.process().selected_thread();
        if thread.stop_reason() == StopReason::Breakpoint {
            let bp_id = thread.stop_reason_data_at_index(0) as BreakpointID;
            let mut breakpoints = self.breakpoints.borrow_mut();

            if let Some(symbol) = self.symbol_from_frame(args.thread_id as ThreadID, args.frame_index) {
                if let Some(bp_info) = breakpoints.breakpoint_infos.get_mut(&bp_id) {
                    bp_info.exclusions.push(symbol.clone());

                    let breakpoint_id = if let BreakpointKind::Exception(exc_name) = &bp_info.kind {
                        let filter = DebugSession::get_exception_filters().iter().find(|f| f.filter == *exc_name);
                        Either::Second((exc_name.clone(), filter.unwrap().label.clone()))
                    } else {
                        Either::First(bp_id as i64)
                    };
                    return Ok(ExcludeCallerResponse { breakpoint_id, symbol });
                }
            }
            bail!(blame_user(str_error("Could not locate symbol for this stack frame.")));
        } else {
            bail!(blame_user(str_error("Must be stopped on a breakpoint.")));
        }
    }

    fn symbol_from_frame(&self, thread_id: ThreadID, frame_index: u32) -> Option<String> {
        if let Some(thread) = self.target.process().thread_by_id(thread_id) {
            let frame = thread.frame_at_index(frame_index);
            let pc = frame.pc_address();
            pc.symbol().map(|s| s.name().to_owned())
        } else {
            None
        }
    }

    pub(super) fn handle_set_excluded_callers(&mut self, args: SetExcludedCallersRequest) -> Result<(), Error> {
        let mut breakpoints = self.breakpoints.borrow_mut();
        for bp_info in breakpoints.breakpoint_infos.values_mut() {
            bp_info.exclusions.clear();
        }
        for exclusion in args.exclusions {
            match exclusion.breakpoint_id {
                Either::First(bp_id) => {
                    let bp_id = bp_id as BreakpointID;
                    if let Some(bp_info) = breakpoints.breakpoint_infos.get_mut(&bp_id) {
                        bp_info.exclusions.push(exclusion.symbol);
                    }
                }
                Either::Second(exc_name) => {
                    for bp_info in breakpoints.breakpoint_infos.values_mut() {
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
