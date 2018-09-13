use super::*;

cpp_class!(pub unsafe struct SBCommandInterpreter as "SBCommandInterpreter");

unsafe impl Send for SBCommandInterpreter {}

impl SBCommandInterpreter {
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBCommandInterpreter*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn handle_command(
        &self, command: &str, result: &mut SBCommandReturnObject, add_to_history: bool,
    ) -> ReturnStatus {
        with_cstr(command, |command| {
            cpp!(unsafe [self as "SBCommandInterpreter*", command as "const char*",
                         result as "SBCommandReturnObject*", add_to_history as "bool"] -> ReturnStatus as "ReturnStatus" {
                return self->HandleCommand(command, *result, add_to_history);
            })
        })
    }
    pub fn handle_command_with_context(
        &self, command: &str, context: &SBExecutionContext, result: &mut SBCommandReturnObject, add_to_history: bool,
    ) -> ReturnStatus {
        with_cstr(command, |command| {
            cpp!(unsafe [self as "SBCommandInterpreter*", command as "const char*", context as "SBExecutionContext*",
                         result as "SBCommandReturnObject*", add_to_history as "bool"] -> ReturnStatus as "ReturnStatus" {
                return self->HandleCommand(command, *context, *result, add_to_history);
            })
        })
    }
    pub fn handle_completions(
        &self, current_line: &str, cursor_pos: u32, match_start_point: u32, max_return_elements: Option<u32>,
    ) -> Vec<String> {
        unsafe {
            let line_start = current_line.as_ptr();
            let line_end = line_start.offset(current_line.len() as isize);
            let cursor = line_start.offset(cursor_pos as isize);
            let match_start_point = match_start_point as c_int;
            let max_return_elements = match max_return_elements {
                Some(n) => n as c_int,
                None => -1
            };
            let mut matches = SBStringList::new();
            cpp!(unsafe [self as "SBCommandInterpreter*",
                     line_start as "const char*", line_end as "const char*",
                     cursor as "const char*", match_start_point as "int",
                     max_return_elements as "int", mut matches as "SBStringList"] -> i32 as "int" {
                return self->HandleCompletion(line_start, cursor, line_end, match_start_point, max_return_elements, matches);
            });
            let result = matches.iter().map(|s| s.to_owned()).collect::<Vec<String>>();
            result
        }
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
