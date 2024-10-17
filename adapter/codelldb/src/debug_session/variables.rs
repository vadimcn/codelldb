use crate::prelude::*;

use crate::expressions::{self, FormatSpec, PreparedExpression};
use crate::handles::{self, Handle};

use super::into_string_lossy;
use super::AsyncResponse;

use adapter_protocol::*;
use futures::future;
use lldb::*;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Write;
use std::time;

pub enum Container {
    StackFrame(SBFrame),
    Locals(SBFrame),
    Statics(SBFrame),
    Globals(SBFrame),
    Registers(SBFrame),
    SBValue(SBValue),
}

impl super::DebugSession {
    pub fn handle_scopes(&mut self, args: ScopesArguments) -> Result<ScopesResponseBody, Error> {
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
            Err(format!("Invalid frame reference: {}", args.frame_id))?
        }
    }

    pub fn handle_variables(&mut self, args: VariablesArguments) -> Result<VariablesResponseBody, Error> {
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
                    let mut variables = self.convert_scope_values(&mut vars_iter, "", Some(container_handle), true)?;
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
                    let mut vars_iter = variables.iter().filter(|v| v.value_type() == ValueType::VariableStatic);
                    self.convert_scope_values(&mut vars_iter, "", Some(container_handle), false)?
                }
                Container::Globals(frame) => {
                    let variables = frame.variables(&VariableOptions {
                        arguments: false,
                        locals: false,
                        statics: true,
                        in_scope_only: true,
                    });
                    let mut vars_iter = variables.iter().filter(|v| v.value_type() == ValueType::VariableGlobal);
                    self.convert_scope_values(&mut vars_iter, "", Some(container_handle), false)?
                }
                Container::Registers(frame) => {
                    let list = frame.registers();
                    let mut vars_iter = list.iter();
                    self.convert_scope_values(&mut vars_iter, "", Some(container_handle), false)?
                }
                Container::SBValue(var) => {
                    let container_eval_name = self.compose_container_eval_name(container_handle);
                    let var = var.clone();
                    let mut vars_iter = var.children();
                    let mut variables =
                        self.convert_scope_values(&mut vars_iter, &container_eval_name, Some(container_handle), false)?;
                    // If synthetic, add [raw] view.
                    if var.is_synthetic() {
                        let raw_var = var.non_synthetic_value();
                        let handle = self.var_refs.create(Some(container_handle), "[raw]", Container::SBValue(raw_var));
                        let raw = Variable {
                            name: "[raw]".to_owned(),
                            value: var.type_name().unwrap_or_default().to_owned(),
                            variables_reference: handles::to_i64(Some(handle)),
                            presentation_hint: Some(presentation_hint(&["readOnly", "virtual"])),
                            ..Default::default()
                        };
                        variables.push(raw);
                    }
                    variables
                }
                Container::StackFrame(_) => vec![],
            };
            Ok(VariablesResponseBody { variables: variables })
        } else {
            Err(format!("Invalid variabes reference: {}", container_handle))?
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
        deduplicate: bool,
    ) -> Result<Vec<Variable>, Error> {
        let mut variables = vec![];
        let mut variables_idx = HashMap::new();

        let start = time::SystemTime::now();
        for var in vars_iter {
            let variable = self.var_to_variable(&var, container_eval_name, container_handle);

            if deduplicate {
                if let Some(idx) = variables_idx.get(&variable.name) {
                    variables[*idx] = variable;
                } else {
                    variables_idx.insert(variable.name.clone(), variables.len());
                    variables.push(variable);
                }
            } else {
                variables.push(variable);
            }

            if self.current_cancellation.is_cancelled() {
                bail!(as_user_error("<cancelled>"));
            }

            // Bail out if timeout has expired.
            if start.elapsed().unwrap_or_default() > self.evaluation_timeout {
                self.console_error("Child list expansion has timed out.");
                variables.push(Variable {
                    name: "[timed out]".to_owned(),
                    type_: Some("Expansion of this list has timed out.".to_owned()),
                    presentation_hint: Some(presentation_hint(&["readOnly", "virtual"])),
                    ..Default::default()
                });
                break;
            }
        }
        Ok(variables)
    }

    // SBValue to VSCode Variable
    fn var_to_variable(
        &mut self,
        var: &SBValue,
        container_eval_name: &str,
        container_handle: Option<Handle>,
    ) -> Variable {
        let name = var.name().unwrap_or_default();
        let dtype = var.display_type_name();
        let value = self.get_var_summary(&var, container_handle.is_some());
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

        let mem_ref = self.get_mem_ref_for_var(var);

        let is_settable = match var.type_().basic_type() {
            BasicType::Char
            | BasicType::SignedChar
            | BasicType::UnsignedChar
            | BasicType::WChar
            | BasicType::SignedWChar
            | BasicType::UnsignedWChar
            | BasicType::Char16
            | BasicType::Char32
            | BasicType::Short
            | BasicType::UnsignedShort
            | BasicType::Int
            | BasicType::UnsignedInt
            | BasicType::Long
            | BasicType::UnsignedLong
            | BasicType::LongLong
            | BasicType::UnsignedLongLong
            | BasicType::Int128
            | BasicType::UnsignedInt128
            | BasicType::Bool
            | BasicType::Half
            | BasicType::Float
            | BasicType::Double
            | BasicType::LongDouble => true,
            _ => false,
        };

        Variable {
            name: name.to_owned(),
            value: value,
            type_: dtype.map(|v| v.to_owned()),
            variables_reference: handles::to_i64(handle),
            evaluate_name: eval_name,
            memory_reference: mem_ref,
            presentation_hint: if is_settable { None } else { Some(presentation_hint(&["readOnly"])) },
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
    pub fn get_var_summary(&self, var: &SBValue, is_container: bool) -> String {
        let err = var.error();
        if err.is_failure() {
            return format!("<{}>", err);
        }

        let mut var = Cow::Borrowed(var);

        if self.deref_pointers
            && var.format() == Format::Default
            && var.type_().type_class().intersects(TypeClass::Pointer | TypeClass::Reference)
        {
            // Rather than showing pointer's numeric value, which is rather uninteresting,
            // we prefer to display summary of the object it points to.
            match self.try_deref_pointer(var.as_ref()) {
                Either::First(summary) => return summary,
                Either::Second(Some(pointee)) => var = Cow::Owned(pointee),
                _ => {}
            }
        }

        let summary = if let Some(summary_str) = var.summary().map(|s| into_string_lossy(s)) {
            summary_str
        } else if let Some(value_str) = var.value().map(|s| into_string_lossy(s)) {
            value_str
        } else if is_container {
            // Try to synthesize a summary from var's children.
            self.get_container_summary(var.as_ref())
        } else {
            // Otherwise give up.
            "<not available>".to_owned()
        };
        summary
    }

    // This may return either a summary string or a pointee
    fn try_deref_pointer(&self, ptr: &SBValue) -> Either<String, Option<SBValue>> {
        // If the pointer has an associated synthetic, or if it's a pointer to a basic
        // type such as `char`, use summary of the pointer itself;
        // otherwise prefer to dereference and use summary of the pointee.
        let pointee_type = ptr.type_().pointee_type();
        if ptr.is_synthetic() || pointee_type.basic_type() != BasicType::Invalid {
            if let Some(value_str) = ptr.summary().map(|s| into_string_lossy(s)) {
                return Either::First(value_str);
            }
        }

        if ptr.value_as_unsigned(0) == 0 {
            return Either::First("<null>".to_owned());
        }

        let pointee = ptr.dereference();
        if pointee.is_valid() {
            if pointee.data().byte_size() > 0 {
                if pointee_type.type_class().intersects(TypeClass::Pointer | TypeClass::Reference) {
                    // If pointee is a pointer too, display its value in curly braces, otherwise it gets rather confusing.
                    if let Some(value_str) = pointee.value().map(|s| into_string_lossy(s)) {
                        return Either::First(format!("{{{}}}", value_str));
                    }
                }
                return Either::Second(Some(pointee)); // The replacement value
            } else {
                return Either::First("<invalid address>".to_owned());
            }
        } else {
            // Could be a void*.  Try reading one byte to determine wheter the address is valid.
            let addr = ptr.value_as_unsigned(0);
            let mut buff = [0u8];
            if let Ok(1) = self.target.process().read_memory(addr, &mut buff) {
                return Either::Second(None);
            } else {
                return Either::First("<invalid address>".to_owned());
            }
        }
    }

    fn get_container_summary(&self, var: &SBValue) -> String {
        let start = time::SystemTime::now();
        let mut summary = String::from("{");
        let mut sep = "";
        for child in var.children() {
            if summary.len() > self.max_summary_length || start.elapsed().unwrap_or_default() > self.summary_timeout {
                log_errors!(write!(summary, "{}...", sep));
                break;
            }

            if let Some(name) = child.name() {
                if let Some(Ok(value)) = child.summary().or_else(|| child.value()).map(|s| s.to_str()) {
                    if name.starts_with("[") {
                        log_errors!(write!(summary, "{}{}", sep, value));
                    } else {
                        log_errors!(write!(summary, "{}{}:{}", sep, name, value));
                    }
                    sep = ", ";
                }
            }
        }

        if summary.len() <= 1 {
            summary.push_str("...");
        }

        summary.push_str("}");
        summary
    }

    // Retrieve the memory reference for a given variable.
    fn get_mem_ref_for_var(&self, var: &SBValue) -> Option<String> {
        match self.client_caps.supports_memory_references {
            Some(true) => {
                match var.value_type() {
                    // If the value is stored in a register, assume it is an address and use it directly.
                    // This allows users to dump memory by referencing registers like SP or RIP+offset.
                    ValueType::Register => Some(format!("0x{:X}", var.value_as_unsigned(0))),
                    _ => {
                        if var.type_().is_pointer_type() {
                            // If the value is a pointer type, treat it as a memory address.
                            // This allows users to dump arbitrary memory addresses, e.g., by using (void*)0x1234.
                            Some(format!("0x{:X}", var.value_as_unsigned(0)))
                        } else {
                            let load_addr = var.load_address();
                            match load_addr {
                                lldb::INVALID_ADDRESS => None,
                                // If the variable has a valid load address, use it as the memory reference.
                                _ => Some(format!("0x{:X}", load_addr)),
                            }
                        }
                    }
                }
            }
            _ => None,
        }
    }

    pub fn handle_evaluate(&mut self, args: EvaluateArguments) -> Result<ResponseBody, Error> {
        let frame = match args.frame_id {
            Some(frame_id) => {
                let handle = handles::from_i64(frame_id)?;
                match self.var_refs.get(handle) {
                    Some(Container::StackFrame(ref frame)) => {
                        // If they had used `frame select` command after the last stop,
                        // use currently selected frame from frame's thread, instead of the frame itself.
                        if self.selected_frame_changed {
                            Some(frame.thread().selected_frame())
                        } else {
                            Some(frame.clone())
                        }
                    }
                    _ => {
                        error!("Invalid frameId");
                        None
                    }
                }
            }
            None => None,
        };

        let context = args.context.as_ref().map(|s| s.as_ref());
        let result = match context {
            Some("repl") => match self.console_mode {
                ConsoleMode::Commands => {
                    if args.expression.starts_with("?") {
                        self.handle_evaluate_expression(&args.expression[1..], frame)
                    } else {
                        self.handle_execute_command(&args.expression, frame, false)
                    }
                }
                ConsoleMode::Split | ConsoleMode::Evaluate => {
                    if args.expression.starts_with('`') {
                        self.handle_execute_command(&args.expression[1..], frame, false)
                    } else if args.expression.starts_with("/cmd ") {
                        self.handle_execute_command(&args.expression[5..], frame, false)
                    } else {
                        self.handle_evaluate_expression(&args.expression, frame)
                    }
                }
            },
            Some("hover") if !self.evaluate_for_hovers => bail!("Hovers are disabled."),
            // out protocol extension for testing
            Some("_command") => self.handle_execute_command(&args.expression, frame, true),
            // "watch"
            _ => self.handle_evaluate_expression(&args.expression, frame),
        };

        // Return async, even though we already have the response,
        // so that evaluator's stdout messages get displayed first.
        let result = result.map(|r| ResponseBody::evaluate(r));
        Err(AsyncResponse(Box::new(future::ready(result))).into())
    }

    fn handle_execute_command(
        &mut self,
        command: &str,
        frame: Option<SBFrame>,
        return_output: bool, // return command output in EvaluateResponseBody::result
    ) -> Result<EvaluateResponseBody, Error> {
        let context = self.context_from_frame(frame.as_ref());
        let mut result = SBCommandReturnObject::new();
        result.set_immediate_output_file(SBFile::from(self.console_pipe.try_clone()?, true))?;
        let interp = self.debugger.command_interpreter();
        let ok = interp.handle_command_with_context(command, &context, &mut result, false);
        debug!("{} -> {:?}, {:?}", command, ok, result);
        // TODO: multiline
        if result.succeeded() {
            Ok(EvaluateResponseBody {
                result: match return_output {
                    true => into_string_lossy(result.output()).trim_end().to_string(),
                    false => String::new(),
                },
                ..Default::default()
            })
        } else {
            let message = into_string_lossy(result.error()).trim_end().to_string();
            bail!(as_user_error(message))
        }
    }

    fn handle_evaluate_expression(
        &mut self,
        expression: &str,
        frame: Option<SBFrame>,
    ) -> Result<EvaluateResponseBody, Error> {
        // Expression
        let (pp_expr, format_spec) =
            expressions::prepare_with_format(expression, self.default_expr_type).map_err(as_user_error)?;

        match self.evaluate_expr_in_frame(&pp_expr, frame.as_ref()) {
            Ok(sbval) => {
                let sbval = self.apply_format_spec(sbval, &format_spec)?;
                let handle = self.get_var_handle(None, expression, &sbval);
                Ok(EvaluateResponseBody {
                    result: self.get_var_summary(&sbval, handle.is_some()),
                    type_: sbval.display_type_name().map(|s| s.to_owned()),
                    variables_reference: handles::to_i64(handle),
                    memory_reference: self.get_mem_ref_for_var(&sbval),
                    ..Default::default()
                })
            }
            Err(err) => Err(err),
        }
    }

    pub fn evaluate_user_supplied_expr(
      &self,
      expression: &str,
      frame: Option<SBFrame>,
    ) -> Result<SBValue, Error> {
        // The format spec is ignored and dropped, because the purpose of this
        // fn is to just return the SBValue to get references for things like
        // data breakpoints
        let (pp_expr, _format_spec) =
            expressions::prepare_with_format(expression, self.default_expr_type).map_err(as_user_error)?;
        self.evaluate_expr_in_frame(&pp_expr, frame.as_ref())
    }


    // Evaluates expr in the context of frame (or in global context if frame is None)
    // Returns expressions.Value or SBValue on success, SBError on failure.
    pub fn evaluate_expr_in_frame(
        &self,
        expression: &PreparedExpression,
        frame: Option<&SBFrame>,
    ) -> Result<SBValue, Error> {
        match (expression, self.python.as_ref()) {
        (PreparedExpression::Native(pp_expr), _) => {
            let result = match frame {
                Some(frame) => frame.evaluate_expression(&pp_expr).into_result(),
                None => self.target.evaluate_expression(&pp_expr).into_result(),
            };
            let result = result.map_err(as_user_error)?;
            Ok(result)
        }
        (PreparedExpression::Python(pp_expr), Some(python)) | //.
        (PreparedExpression::Simple(pp_expr), Some(python)) => {
            let context = self.context_from_frame(frame);
            let pycode = python.compile_code(pp_expr, "<input>").map_err(as_user_error)?;
            let is_simple_expr = matches!(expression, PreparedExpression::Simple(_));
            let result = python.evaluate(&pycode, is_simple_expr, &context).map_err(as_user_error)?;
            Ok(result)
        }
        _ => bail!(as_user_error("Python expressions are disabled.")),
    }
    }

    pub fn handle_set_variable(&mut self, args: SetVariableArguments) -> Result<SetVariableResponseBody, Error> {
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
                        value: self.get_var_summary(&child, handle.is_some()),
                        type_: child.type_name().map(|s| s.to_owned()),
                        variables_reference: Some(handles::to_i64(handle)),
                        named_variables: None,
                        indexed_variables: None,
                    };
                    Ok(response)
                }
                Err(err) => Err(as_user_error(err))?,
            }
        } else {
            bail!(as_user_error("Could not set variable value."));
        }
    }

    pub fn apply_format_spec(&self, sbval: SBValue, format_spec: &FormatSpec) -> Result<SBValue, Error> {
        let mut sbval = sbval;
        if let Some(size) = format_spec.array {
            let var_type = sbval.type_();
            let type_class = var_type.type_class();
            sbval = if type_class.intersects(TypeClass::Pointer | TypeClass::Reference) {
                // For pointers and references we re-interpret the pointee.
                let array_type = var_type.pointee_type().array_type(size as u64);
                let pointee = sbval.dereference().into_result().map_err(as_user_error)?;
                let addr = pointee.address().ok_or_else(|| as_user_error("No address"))?;
                sbval.target().create_value_from_address("(as array)", &addr, &array_type)
            } else if type_class.intersects(TypeClass::Array) {
                // For arrays, re-interpret the array length.
                let array_type = var_type.array_element_type().array_type(size as u64);
                let addr = sbval.address().ok_or_else(|| as_user_error("No address"))?;
                sbval.target().create_value_from_address("(as array)", &addr, &array_type)
            } else {
                // For other types re-interpret the value itself.
                let array_type = var_type.array_type(size as u64);
                let addr = sbval.address().ok_or_else(|| as_user_error("No address"))?;
                sbval.target().create_value_from_address("(as array)", &addr, &array_type)
            };
        }
        sbval.set_format(format_spec.format.unwrap_or(self.global_format));
        Ok(sbval)
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

fn presentation_hint(attributes: &[&str]) -> VariablePresentationHint {
    VariablePresentationHint {
        attributes: Some(attributes.iter().map(|a| (*a).into()).collect()),
        ..Default::default()
    }
}
