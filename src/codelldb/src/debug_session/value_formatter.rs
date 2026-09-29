use crate::cancellation;
use crate::expressions::FormatSpec;
use crate::prelude::*;

use super::{into_string_lossy, DebugSession};
use adapter_protocol::Either;
use lldb::*;
use std::borrow::Cow;
use std::fmt::Write;
use std::time;

pub(super) struct ValueFormatter {
    target: SBTarget,
    global_format: Format,
    deref_pointers: bool,
    summary_timeout: time::Duration,
    max_summary_length: usize,
    current_cancellation: Option<cancellation::Receiver>,
}

impl ValueFormatter {
    pub(super) fn from_session(session: &DebugSession, current_cancellation: Option<cancellation::Receiver>) -> Self {
        Self {
            target: session.target.clone(),
            global_format: session.global_format,
            deref_pointers: session.deref_pointers,
            summary_timeout: session.summary_timeout,
            max_summary_length: session.max_summary_length,
            current_cancellation,
        }
    }

    // Get displayable string from an SBValue.
    pub(super) fn get_var_summary(&self, var: &SBValue, unlimited: bool) -> String {
        let var_type = var.type_();

        // Special case for Rust's unit type as LLDB returns an error for instances of void.
        if var_type.basic_type() == BasicType::Void {
            return "()".into();
        }

        let err = var.error();
        if err.is_failure() {
            return format!("<{}>", err);
        }

        let mut var = Cow::Borrowed(var);

        // Rather than showing the pointer's numeric value, which is rather uninteresting, we display a summary
        // of the object it points to (if deref_pointers is enabled).
        if self.deref_pointers
            && var.format() == Format::Default
            && var_type.type_class().intersects(TypeClass::Pointer | TypeClass::Reference)
        {
            match self.try_deref_pointer(var.as_ref()) {
                Either::First(summary) => return summary,
                Either::Second(Some(pointee)) => var = Cow::Owned(pointee),
                _ => {}
            }
        }

        if let Some(summary_str) = var.summary().map(|s| into_string_lossy(s)) {
            summary_str
        } else if let Some(value_str) = var.value().map(|s| into_string_lossy(s)) {
            value_str
        } else if var.might_have_children() && var.num_children() > 0 {
            self.get_container_summary(var.as_ref(), unlimited)
        } else {
            "<not available>".to_owned()
        }
    }

    // This may return either a summary string or a pointee.
    fn try_deref_pointer(&self, ptr: &SBValue) -> Either<String, Option<SBValue>> {
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
                    if let Some(value_str) = pointee.value().map(|s| into_string_lossy(s)) {
                        return Either::First(format!("{{{}}}", value_str));
                    }
                }
                return Either::Second(Some(pointee));
            } else {
                return Either::First("<invalid address>".to_owned());
            }
        } else {
            // A void pointer may not dereference, but its address could still be readable.
            let addr = ptr.value_as_unsigned(0);
            let mut buff = [0u8];
            if let Ok(1) = self.target.process().read_memory(addr, &mut buff) {
                return Either::Second(None);
            } else {
                return Either::First("<invalid address>".to_owned());
            }
        }
    }

    // unlimited: if true, return the full summary without truncating or timing out.
    fn get_container_summary(&self, var: &SBValue, unlimited: bool) -> String {
        let start = time::SystemTime::now();
        let mut summary = String::from("{");
        let mut sep = "";
        for child in var.children() {
            let should_stop = self.current_cancellation.as_ref().map_or(false, |cancel| cancel.is_cancelled())
                || (!unlimited
                    && (summary.len() > self.max_summary_length
                        || start.elapsed().unwrap_or_default() > self.summary_timeout));
            if should_stop {
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

    pub(super) fn apply_format_spec(&self, sbval: SBValue, format_spec: &FormatSpec) -> Result<SBValue, Error> {
        let mut sbval = sbval;
        if let Some(size) = format_spec.array {
            let var_type = sbval.type_();
            let type_class = var_type.type_class();
            sbval = if type_class.intersects(TypeClass::Pointer | TypeClass::Reference) {
                // For pointers and references we re-interpret the pointee.
                let array_type = var_type.pointee_type().array_type(size as u64);
                let pointee = sbval.dereference().into_result()?;
                let addr = pointee.address().ok_or_else(|| str_error("No address"))?;
                sbval.target().create_value_from_address("(as array)", &addr, &array_type)
            } else if type_class.intersects(TypeClass::Array) {
                // For arrays, re-interpret the array length.
                let array_type = var_type.array_element_type().array_type(size as u64);
                let addr = sbval.address().ok_or_else(|| str_error("No address"))?;
                sbval.target().create_value_from_address("(as array)", &addr, &array_type)
            } else {
                // For other types re-interpret the value itself.
                let array_type = var_type.array_type(size as u64);
                let addr = sbval.address().ok_or_else(|| str_error("No address"))?;
                sbval.target().create_value_from_address("(as array)", &addr, &array_type)
            };
        }
        sbval.set_format(format_spec.format.unwrap_or(self.global_format));
        Ok(sbval)
    }
}
