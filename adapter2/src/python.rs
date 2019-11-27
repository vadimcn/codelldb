use crate::error::Error;
use lldb::*;
use log::*;
use std::env;
use std::ffi::CStr;
use std::mem;
use std::os::raw::{c_char, c_int, c_long, c_void};

pub trait EventSink {
    fn display_html(&self, html: String, title: Option<String>, position: Option<i32>, reveal: bool);
}

#[repr(C)]
union ValueResult {
    value: mem::ManuallyDrop<SBValue>,
    error: mem::ManuallyDrop<SBError>,
}

#[repr(C)]
union BoolResult {
    value: bool,
    error: mem::ManuallyDrop<SBError>,
}

pub struct PythonInterface {
    initialized: bool,
    event_sink: Box<dyn EventSink + Send>,
    evaluate_ptr: Option<
        extern "C" fn(
            result: *mut ValueResult,
            expr: *const c_char,
            expr_len: usize,
            is_simple_expr: bool,
            context: SBExecutionContext,
        ) -> i32,
    >,
    evaluate_as_bool_ptr: Option<
        extern "C" fn(
            result: *mut BoolResult,
            expr: *const c_char,
            expr_len: usize,
            is_simple_expr: bool,
            context: SBExecutionContext,
        ) -> i32,
    >,
    modules_loaded_ptr: Option<extern "C" fn(modules: *const SBModule, modules_len: usize) -> i32>,
    shutdown_ptr: Option<extern "C" fn() -> i32>,
}

impl PythonInterface {
    pub fn new(
        interpreter: SBCommandInterpreter,
        event_sink: Box<dyn EventSink + Send>,
    ) -> Result<Box<PythonInterface>, Error> {
        let current_exe = env::current_exe()?;
        let mut command_result = SBCommandReturnObject::new();

        // Import debugger.py into script interpreter's namespace.
        // This also adds our bin directory to sys.path, so we can import the rest of the modules below.
        let init_script = current_exe.with_file_name("debugger.py");
        let command = format!("command script import '{}'", init_script.to_str().unwrap());
        interpreter.handle_command(&command, &mut command_result, false);
        if !command_result.succeeded() {
            return Err(Error::Internal(format!("{:?}", command_result)));
        }

        let mut interface = Box::new(PythonInterface {
            initialized: false,
            shutdown_ptr: None,
            evaluate_ptr: None,
            evaluate_as_bool_ptr: None,
            modules_loaded_ptr: None,
            event_sink,
        });

        unsafe extern "C" fn init_callback(
            interface: *mut PythonInterface,
            shutdown_ptr: *const c_void,
            evaluate_ptr: *const c_void,
            evaluate_as_bool_ptr: *const c_void,
            modules_loaded_ptr: *const c_void,
        ) {
            (*interface).shutdown_ptr = Some(mem::transmute(shutdown_ptr));
            (*interface).evaluate_ptr = Some(mem::transmute(evaluate_ptr));
            (*interface).evaluate_as_bool_ptr = Some(mem::transmute(evaluate_as_bool_ptr));
            (*interface).modules_loaded_ptr = Some(mem::transmute(modules_loaded_ptr));
            (*interface).initialized = true;
        }

        unsafe extern "C" fn display_html_callback(
            interface: *mut PythonInterface,
            html: *const c_char,
            title: *const c_char,
            position: c_int,
            reveal: c_int,
        ) {
            if html.is_null() {
                return;
            }
            let html = CStr::from_ptr(html).to_str().unwrap().to_string();
            let title = if title.is_null() {
                None
            } else {
                Some(CStr::from_ptr(title).to_str().unwrap().to_string())
            };
            let position = Some(position as i32);
            let reveal = reveal != 0;
            (*interface).event_sink.display_html(html, title, position, reveal);
        }

        let command = format!(
            "script import codelldb; codelldb.initialize({:p}, {:p}, {:p})",
            init_callback as *const c_void, display_html_callback as *const c_void, interface
        );
        interpreter.handle_command(&command, &mut command_result, false);
        if !command_result.succeeded() {
            return Err(Error::Internal(format!("{:?}", command_result)));
        }

        // Make sure Python side has called us back.
        if !interface.initialized {
            return Err(Error::Internal("Could not initialize Python environment.".into()));
        }

        let rust_formatters = current_exe.with_file_name("rust.py");
        let command = format!("command script import '{}'", rust_formatters.to_str().unwrap()); /*###*/
        interpreter.handle_command(&command, &mut command_result, false);
        if !command_result.succeeded() {
            error!("{:?}", command_result); // But carry on - Rust formatters are not critical to have.
        }

        Ok(interface)
    }

    pub fn evaluate(&self, expr: &str, is_simple_expr: bool, context: &SBExecutionContext) -> Result<SBValue, String> {
        unsafe {
            let expt_ptr = expr.as_ptr() as *const c_char;
            let expr_size = expr.len();
            let mut result = mem::MaybeUninit::<ValueResult>::uninit();
            let status =
                (self.evaluate_ptr.unwrap())(result.as_mut_ptr(), expt_ptr, expr_size, is_simple_expr, context.clone());
            if status > 0 {
                Ok(mem::ManuallyDrop::into_inner(result.assume_init().value))
            } else if status < 0 {
                Err(result.assume_init().error.to_string())
            } else {
                Err("Evaluation failed".into())
            }
        }
    }

    pub fn evaluate_as_bool(
        &self,
        expr: &str,
        is_simple_expr: bool,
        context: &SBExecutionContext,
    ) -> Result<bool, String> {
        unsafe {
            let expt_ptr = expr.as_ptr() as *const c_char;
            let expr_size = expr.len();
            let mut result = mem::MaybeUninit::<BoolResult>::uninit();
            let status = (self.evaluate_as_bool_ptr.unwrap())(
                result.as_mut_ptr(),
                expt_ptr,
                expr_size,
                is_simple_expr,
                context.clone(),
            );
            if status > 0 {
                Ok(result.assume_init().value)
            } else if status < 0 {
                Err(result.assume_init().error.to_string())
            } else {
                Err("Evaluation failed".into())
            }
        }
    }

    pub fn modules_loaded(&self, modules: &mut dyn Iterator<Item = &SBModule>) {
        let modules = modules.cloned().collect::<Vec<SBModule>>();
        (self.modules_loaded_ptr.unwrap())(modules.as_ptr(), modules.len());
    }
}

impl Drop for PythonInterface {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { (self.shutdown_ptr.unwrap())() };
        }
    }
}

#[test]
fn test_sizeof() {
    // codelldb.py makes assumptions about sizes of these types:
    assert_eq!(mem::size_of::<SBError>(), 8);
    assert_eq!(mem::size_of::<SBExecutionContext>(), 16);
    assert_eq!(mem::size_of::<SBValue>(), 16);
    assert_eq!(mem::size_of::<SBModule>(), 16);
}
