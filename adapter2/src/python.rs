use log::{debug, error, info};
use std::env;
use std::fmt::Write;
use std::os::raw::{c_int, c_void};
use std::slice;

use lldb::*;

use crate::error::Error;

pub fn initialize(interpreter: &SBCommandInterpreter) -> Result<(), Error> {
    let mut init_script = env::current_exe()?;
    init_script.set_file_name("codelldb.py");

    let mut command_result = SBCommandReturnObject::new();
    let command = format!("command script import '{}'", init_script.to_str()?);
    interpreter.handle_command(&command, &mut command_result, false);
    info!("{:?}", command_result);

    let mut rust_formatters = init_script.clone();
    rust_formatters.set_file_name("rust.py");
    let command = format!("command script import '{}'", rust_formatters.to_str()?);
    interpreter.handle_command(&command, &mut command_result, false);
    info!("{:?}", command_result);

    Ok(())
}

#[derive(Debug)]
pub enum PythonValue {
    SBValue(SBValue),
    Int(i64),
    Bool(bool),
    String(String),
    Object(String),
}

type EvalResult = Result<PythonValue, String>;

pub fn evaluate(
    interpreter: &SBCommandInterpreter, script: &str, simple_expr: bool, context: &SBExecutionContext,
) -> EvalResult {
    extern "C" fn callback(ty: c_int, pdata: *const c_void, idata: usize, result_ptr: *mut EvalResult) {
        unsafe {
            *result_ptr = match ty {
                0 => {
                    // Error
                    let bytes = slice::from_raw_parts(pdata as *const u8, idata);
                    Err(String::from_utf8_lossy(bytes).into_owned())
                }
                1 => {
                    // SBValue
                    let sbvalue = pdata as *const SBValue;
                    Ok(PythonValue::SBValue((*sbvalue).clone()))
                }
                2 => {
                    // bool
                    let value = (idata as i64) != 0;
                    Ok(PythonValue::Bool(value))
                }
                3 => {
                    // int
                    let value = idata as i64;
                    Ok(PythonValue::Int(value))
                }
                4 => {
                    // string
                    let bytes = slice::from_raw_parts(pdata as *const u8, idata);
                    Ok(PythonValue::String(String::from_utf8_lossy(bytes).into_owned()))
                }
                5 => {
                    // str(object)
                    let bytes = slice::from_raw_parts(pdata as *const u8, idata);
                    Ok(PythonValue::Object(String::from_utf8_lossy(bytes).into_owned()))
                }
                _ => unreachable!(),
            }
        }
    }

    let mut eval_result = Err(String::new());

    let command = format!(
        "script codelldb.evaluate('{}',{},{:#X},{:#X})",
        script,
        if simple_expr {
            "True"
        } else {
            "False"
        },
        callback as *mut c_void as usize,
        &mut eval_result as *mut EvalResult as usize
    );

    let mut command_result = SBCommandReturnObject::new();
    interpreter.handle_command_with_context(&command, &context, &mut command_result, false);

    info!("{:?}", command_result);
    info!("{:?}", eval_result);
    eval_result
}

pub fn modules_loaded(interpreter: &SBCommandInterpreter, modules: &mut Iterator<Item = &SBModule>) {
    extern "C" fn assign_sbmodule(dest: *mut SBModule, src: *const SBModule) {
        unsafe {
            *dest = (*src).clone();
        }
    }

    let module_addrs = modules.fold(String::new(), |mut s, m| {
        if !s.is_empty() {
            s.push(',');
        }
        write!(s, "{:#X}", m as *const SBModule as usize);
        s
    });
    info!("{}", module_addrs);

    let mut command_result = SBCommandReturnObject::new();
    let command =
        format!("script codelldb.modules_loaded([{}],{:#X})", module_addrs, assign_sbmodule as *mut c_void as usize,);
    interpreter.handle_command(&command, &mut command_result, false);
    debug!("{:?}", command_result);
}
