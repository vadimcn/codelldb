#![allow(non_camel_case_types)]

use serde::{Deserialize, Serialize};

schemafy::schemafy!("src/debugAdapterProtocol.json");

impl Default for Breakpoint {
    fn default() -> Self {
        Breakpoint {
            id: None,
            verified: false,
            column: None,
            end_column: None,
            line: None,
            end_line: None,
            message: None,
            source: None,
            instruction_reference: None,
            offset: None,
            reason: None,
        }
    }
}

impl Default for StackFrame {
    fn default() -> Self {
        StackFrame {
            id: 0,
            name: String::new(),
            source: None,
            line: 0,
            column: 0,
            end_column: None,
            end_line: None,
            can_restart: None,
            instruction_pointer_reference: None,
            module_id: None,
            presentation_hint: None,
        }
    }
}

impl Default for Scope {
    fn default() -> Self {
        Scope {
            column: None,
            end_column: None,
            end_line: None,
            expensive: false,
            indexed_variables: None,
            line: None,
            name: String::new(),
            named_variables: None,
            presentation_hint: None,
            source: None,
            variables_reference: 0,
        }
    }
}

impl Default for Variable {
    fn default() -> Self {
        Variable {
            name: String::new(),
            value: String::new(),
            variables_reference: 0,
            type_: None,
            evaluate_name: None,
            indexed_variables: None,
            named_variables: None,
            memory_reference: None,
            presentation_hint: None,
            declaration_location_reference: None,
            value_location_reference: None,
        }
    }
}

impl Default for StoppedEventBody {
    fn default() -> Self {
        StoppedEventBody {
            thread_id: None,
            reason: String::new(),
            all_threads_stopped: None,
            description: None,
            preserve_focus_hint: None,
            text: None,
            hit_breakpoint_ids: None,
        }
    }
}

impl Default for EvaluateResponseBody {
    fn default() -> Self {
        EvaluateResponseBody {
            result: String::new(),
            type_: None,
            variables_reference: 0,
            indexed_variables: None,
            memory_reference: None,
            value_location_reference: None,
            named_variables: None,
            presentation_hint: None,
        }
    }
}

impl Default for OutputEventBody {
    fn default() -> Self {
        OutputEventBody {
            output: String::new(),
            category: None,
            data: None,
            line: None,
            column: None,
            source: None,
            variables_reference: None,
            location_reference: None,
            group: None,
        }
    }
}

impl Default for CompletionItem {
    fn default() -> Self {
        CompletionItem {
            label: String::new(),
            length: None,
            start: None,
            text: None,
            detail: None,
            type_: None,
            sort_text: None,
            selection_start: None,
            selection_length: None,
        }
    }
}

impl Default for Module {
    fn default() -> Self {
        Module {
            name: String::new(),
            id: serde_json::Value::Null,
            path: None,
            address_range: None,
            date_time_stamp: None,
            is_optimized: None,
            is_user_code: None,
            symbol_file_path: None,
            symbol_status: None,
            version: None,
        }
    }
}

impl Default for DataBreakpointInfoResponseBody {
    fn default() -> Self {
        DataBreakpointInfoResponseBody {
            data_id: None,
            access_types: None,
            can_persist: None,
            description: String::new(),
        }
    }
}

impl Default for ExceptionBreakpointsFilter {
    fn default() -> Self {
        ExceptionBreakpointsFilter {
            filter: String::new(),
            label: String::new(),
            description: None,
            default: None,
            supports_condition: None,
            condition_description: None,
        }
    }
}

impl Default for DisassembledInstruction {
    fn default() -> Self {
        DisassembledInstruction {
            address: String::new(),
            instruction: String::new(),
            instruction_bytes: None,
            symbol: None,
            location: None,
            line: None,
            end_line: None,
            column: None,
            end_column: None,
            presentation_hint: None,
        }
    }
}

impl Default for StepInTarget {
    fn default() -> Self {
        StepInTarget {
            id: 0,
            label: String::new(),
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        }
    }
}

impl Default for SetVariableResponseBody {
    fn default() -> Self {
        SetVariableResponseBody {
            value: String::new(),
            type_: None,
            variables_reference: None,
            named_variables: None,
            indexed_variables: None,
            memory_reference: None,
            value_location_reference: None,
        }
    }
}
