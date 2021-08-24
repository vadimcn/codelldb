use crate::debug_protocol::*;
use crate::disassembly;
use crate::expressions::{self, FormatSpec, HitCondition, PreparedExpression};
use crate::fsutil::normalize_path;
use crate::handles::{self, Handle};
use crate::prelude::*;
use crate::python::PyObject;

use std::collections::HashMap;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use lldb::*;

#[derive(Debug, Clone)]
enum BreakpointKind {
    Source,
    Disassembly,
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

pub(super) struct BreakpointsState {
    source: HashMap<PathBuf, HashMap<i64, BreakpointID>>,
    assembly: HashMap<Handle, HashMap<i64, BreakpointID>>,
    function: HashMap<String, BreakpointID>,
    breakpoint_infos: HashMap<BreakpointID, BreakpointInfo>,
}

impl BreakpointsState {
    pub(super) fn new() -> Self {
        BreakpointsState {
            source: HashMap::new(),
            assembly: HashMap::new(),
            function: HashMap::new(),
            breakpoint_infos: HashMap::new(),
        }
    }
}

impl super::DebugSession {
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
            .and_then(|source_ref| self.disassembly.find_by_handle(source_ref));

        let breakpoints = match (dasm, args.source.adapter_data, args.source.path.as_ref()) {
            (Some(dasm), _, _) => self.set_dasm_breakpoints(dasm, requested_bps),
            (None, Some(adapter_data), _) => self.set_new_dasm_breakpoints(
                &serde_json::from_value::<disassembly::AdapterData>(adapter_data)?,
                requested_bps,
            ),
            (None, None, Some(path)) => self.set_source_breakpoints(Path::new(path), requested_bps),
            _ => bail!("Unexpected"),
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
            // Find an existing breakpoint or create a new one
            let bp = match existing_bps.get(&req.line).and_then(|bp_id| self.target.find_breakpoint_by_id(*bp_id)) {
                Some(bp) => bp,
                None => {
                    self.target.breakpoint_create_by_location(file_path_norm.to_str().ok_or("path")?, req.line as u32)
                }
            };

            let bp_info = BreakpointInfo {
                id: bp.id(),
                breakpoint: bp,
                kind: BreakpointKind::Source,
                condition: req.condition.clone(),
                log_message: req.log_message.clone(),
                hit_condition: self.parse_hit_condition(req.hit_condition.as_ref()),
                hit_count: 0,
            };

            self.init_bp_actions(&bp_info);
            result.push(self.make_bp_response(&bp_info, false));
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

            // Find an existing breakpoint or create a new one
            let bp = match existing_bps.get(&req.line).and_then(|bp_id| self.target.find_breakpoint_by_id(*bp_id)) {
                Some(bp) => bp,
                None => self.target.breakpoint_create_by_absolute_address(laddress),
            };

            let bp_info = BreakpointInfo {
                id: bp.id(),
                breakpoint: bp,
                kind: BreakpointKind::Disassembly,
                condition: req.condition.clone(),
                log_message: req.log_message.clone(),
                hit_condition: self.parse_hit_condition(req.hit_condition.as_ref()),
                hit_count: 0,
            };
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
        let mut new_bps = HashMap::new();
        let mut result = vec![];
        let line_addresses = disassembly::DisassembledRange::lines_from_adapter_data(adapter_data);
        for req in requested_bps {
            let address = line_addresses[req.line as usize] as Address;
            let bp = self.target.breakpoint_create_by_absolute_address(address);
            let bp_info = BreakpointInfo {
                id: bp.id(),
                breakpoint: bp,
                kind: BreakpointKind::Disassembly,
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

    pub(super) fn handle_set_function_breakpoints(
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

        Ok(SetBreakpointsResponseBody {
            breakpoints: result,
        })
    }

    pub(super) fn handle_set_exception_breakpoints(
        &mut self,
        args: SetExceptionBreakpointsArguments,
    ) -> Result<(), Error> {
        let mut breakpoints = self.breakpoints.borrow_mut();
        breakpoints.breakpoint_infos.retain(|_id, bp_info| {
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

    // Generates debug_protocol::Breakpoint message from BreakpointInfo
    fn make_bp_response(&self, bp_info: &BreakpointInfo, include_source: bool) -> Breakpoint {
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

    fn set_exception_breakpoints(&mut self, filters: &[String]) -> Vec<SBBreakpoint> {
        let cpp_throw = filters.iter().any(|x| x == "cpp_throw");
        let cpp_catch = filters.iter().any(|x| x == "cpp_catch");
        let rust_panic = filters.iter().any(|x| x == "rust_panic");
        let mut bps = vec![];
        if cpp_throw || cpp_catch {
            let bp = self.target.breakpoint_create_for_exception(LanguageType::C_plus_plus, cpp_catch, cpp_throw);
            bp.add_name("cpp_exception");
            bps.push(bp);
        }
        if rust_panic {
            let bp = self.target.breakpoint_create_by_name("rust_panic");
            bp.add_name("rust_panic");
            bps.push(bp);
        }
        bps
    }

    // Propagate breakpoint options from BreakpointInfo into the associated SBBreakpoint.
    fn init_bp_actions(&self, bp_info: &BreakpointInfo) {
        // Determine the conditional expression type.
        let py_condition: Option<(PyObject, bool)> = if let Some(ref condition) = bp_info.condition {
            let condition = condition.trim();
            if !condition.is_empty() {
                let pp_expr = expressions::prepare(condition, self.default_expr_type);
                match &pp_expr {
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
                                    let is_simple_expr = matches!(pp_expr, PreparedExpression::Simple(_));
                                    Some((pycode, is_simple_expr))
                                }
                                Err(err) => {
                                    self.console_error(err.to_string());
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        let hit_condition = bp_info.hit_condition.clone();
        let shared_session = self.self_ref.clone();
        bp_info.breakpoint.set_callback(move |process, thread, location| {
            debug!("Callback for breakpoint location {:?}", location);
            tokio::runtime::Handle::current().block_on(
                shared_session.map(|s| s.on_breakpoint_hit(process, thread, location, &py_condition, &hit_condition)),
            )
        });
    }

    fn on_breakpoint_hit(
        &self,
        _process: &SBProcess,
        thread: &SBThread,
        location: &SBBreakpointLocation,
        py_condition: &Option<(PyObject, bool)>,
        hit_condition: &Option<HitCondition>,
    ) -> bool {
        let mut breakpoints = self.breakpoints.borrow_mut();
        let bp_info = breakpoints.breakpoint_infos.get_mut(&location.breakpoint().id()).unwrap();

        if let Some((pycode, is_simple_expr)) = py_condition {
            let frame = thread.frame_at_index(0);
            let context = self.context_from_frame(Some(&frame));
            // TODO: pass bpno
            let should_stop = match &self.python {
                Some(python) => match python.evaluate_as_bool(&pycode, *is_simple_expr, &context) {
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
            let str_val = self.get_var_summary(&sbval, format, sbval.num_children() > 0);
            Ok(str_val)
        })
    }

    pub(super) fn handle_breakpoint_event(&mut self, event: &SBBreakpointEvent) {
        let bp = event.breakpoint();
        let event_type = event.event_type();
        let mut breakpoints = self.breakpoints.borrow_mut();

        if event_type.intersects(BreakpointEventType::Added) {
            // Don't notify if we already are tracking this one.
            if let None = breakpoints.breakpoint_infos.get(&bp.id()) {
                let bp_info = BreakpointInfo {
                    id: bp.id(),
                    breakpoint: bp,
                    kind: BreakpointKind::Source,
                    condition: None,
                    log_message: None,
                    hit_condition: None,
                    hit_count: 0,
                };
                self.send_event(EventBody::breakpoint(BreakpointEventBody {
                    reason: "new".into(),
                    breakpoint: self.make_bp_response(&bp_info, true),
                }));
                breakpoints.breakpoint_infos.insert(bp_info.id, bp_info);
            }
        } else if event_type.intersects(BreakpointEventType::LocationsAdded) {
            if let Some(bp_info) = breakpoints.breakpoint_infos.get_mut(&bp.id()) {
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
            if let Some(_bp_info) = breakpoints.breakpoint_infos.get_mut(&bp.id()) {
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
}
