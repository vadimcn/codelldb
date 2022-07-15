use super::*;

cpp_class!(pub unsafe struct SBCommandInterpreter as "SBCommandInterpreter");

unsafe impl Send for SBCommandInterpreter {}

impl SBCommandInterpreter {
    pub fn debugger(&self) -> SBDebugger {
        cpp!(unsafe [self as "SBCommandInterpreter*"] -> SBDebugger as "SBDebugger" {
            return self->GetDebugger();
        })
    }
    pub fn handle_command(
        &self,
        command: &str,
        result: &mut SBCommandReturnObject,
        add_to_history: bool,
    ) -> ReturnStatus {
        with_cstr(command, |command| {
            cpp!(unsafe [self as "SBCommandInterpreter*", command as "const char*",
                         result as "SBCommandReturnObject*", add_to_history as "bool"] -> ReturnStatus as "ReturnStatus" {
                return self->HandleCommand(command, *result, add_to_history);
            })
        })
    }
    pub fn handle_command_with_context(
        &self,
        command: &str,
        context: &SBExecutionContext,
        result: &mut SBCommandReturnObject,
        add_to_history: bool,
    ) -> ReturnStatus {
        with_cstr(command, |command| {
            cpp!(unsafe [self as "SBCommandInterpreter*", command as "const char*", context as "SBExecutionContext*",
                         result as "SBCommandReturnObject*", add_to_history as "bool"] -> ReturnStatus as "ReturnStatus" {
                return self->HandleCommand(command, *context, *result, add_to_history);
            })
        })
    }
    pub fn handle_completions(
        &self,
        current_line: &str,
        cursor_pos: u32,
        range: Option<(u32, u32)>,
    ) -> Option<(String, Vec<String>)> {
        unsafe {
            let line_start = if current_line.is_empty() {
                ptr::null() // "".as_ptr() returns 0x00000001, which doesn't sit well with HandleCompletion()
            } else {
                current_line.as_ptr()
            };
            let line_end = line_start.offset(current_line.len() as isize);
            let cursor = line_start.offset(cursor_pos as isize);
            let (first_match, max_matches) = match range {
                None => (0, -1),
                Some((first_match, max_matches)) => (first_match as c_int, max_matches as c_int),
            };
            let mut matches = SBStringList::new();
            let rc = cpp!(unsafe [self as "SBCommandInterpreter*",
                     line_start as "const char*", line_end as "const char*", cursor as "const char*",
                     first_match as "int", max_matches as "int",
                     mut matches as "SBStringList"] -> i32 as "int" {
                return self->HandleCompletion(line_start, cursor, line_end, first_match, max_matches, matches);
            });
            if rc <= 0 || matches.len() == 0 {
                None
            } else {
                let mut iter = matches.iter();
                let common_continuation = iter.next().unwrap().to_owned();
                let completions = iter.map(|s| s.to_owned()).collect::<Vec<String>>();
                Some((common_continuation, completions))
            }
        }
    }
}

impl IsValid for SBCommandInterpreter {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBCommandInterpreter*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum ReturnStatus {
    Invalid = 0,
    SuccessFinishNoResult = 1,
    SuccessFinishResult = 2,
    SuccessContinuingNoResult = 3,
    SuccessContinuingResult = 4,
    Started = 5,
    Failed = 6,
    Quit = 7,
}
