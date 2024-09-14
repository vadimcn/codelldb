use crate::prelude::*;

use crate::must_initialize::{Initialized, MustInitialize, NotInitialized};
use adapter_protocol::EventBody;
use lldb::*;

use std::ffi::CStr;
use std::mem;
use std::os::raw::{c_char, c_void};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

#[repr(C, i32)]
#[allow(dead_code)]
enum PyResult<T> {
    Invalid,
    Ok(T),
    Err(SBError),
}

#[repr(C)]
pub struct PyObject {
    object: *mut c_void,
    destructor: unsafe extern "C" fn(*mut c_void),
}

impl Drop for PyObject {
    fn drop(&mut self) {
        unsafe { (self.destructor)(self.object) }
    }
}

unsafe impl Send for PyObject {}

// Interface through which the rest of CodeLLDB interacts with Python.
pub struct PythonInterface {
    initialized: bool,
    interpreter: SBCommandInterpreter,
    adapter_dir: PathBuf,
    event_sender: mpsc::Sender<EventBody>,
    postinit_ptr: MustInitialize<unsafe extern "C" fn(console_fd: usize) -> bool>,
    handle_message_ptr:
        MustInitialize<unsafe extern "C" fn(json: *const c_char, json_len: usize, debugger: SBDebugger) -> bool>,
    compile_code_ptr: MustInitialize<
        unsafe extern "C" fn(
            result: *mut PyResult<*mut c_void>,
            expr: *const c_char,
            expr_len: usize,
            filename: *const c_char,
            filename_len: usize,
        ) -> bool,
    >,
    evaluate_ptr: MustInitialize<
        unsafe extern "C" fn(
            result: *mut PyResult<SBValue>,
            code: *mut c_void,
            is_simple_expr: bool,
            context: SBExecutionContext,
        ) -> bool,
    >,
    evaluate_as_bool_ptr: MustInitialize<
        unsafe extern "C" fn(
            result: *mut PyResult<bool>,
            pycode: *mut c_void,
            is_simple_expr: bool,
            context: SBExecutionContext,
        ) -> bool,
    >,
    execute_in_instance_ptr: MustInitialize<
        unsafe extern "C" fn(result: *mut PyResult<()>, pycode: *mut c_void, debugger: SBDebugger) -> bool,
    >,
    modules_loaded_ptr: MustInitialize<unsafe extern "C" fn(modules: *const SBModule, modules_len: usize) -> bool>,
    drop_pyobject_ptr: MustInitialize<unsafe extern "C" fn(obj: *mut c_void)>,
    shutdown_ptr: MustInitialize<unsafe extern "C" fn() -> bool>,
}

// Initialize Python interface.
// In order to maintain compatibility with Python 2 (in case we need to load an older liblldb),
// we eschew Python C API, preferring to interact with it via `ctypes` module:
// - We use SBCommandInterpreter to import `codelldb` module and to invoke `initialize` function,
//   passing it pointers to callbacks and data on Rust side, the `init_callback` function among them.
// - `codelldb.initialize` invokes `init_callback` with pointers to C ABI stubs wrapping Python side callbacks,
//    which are saved and later used to invoke Python code directly, bypassing the slow SBCommandInterpreter API.
// - If any of the above fails, we declare Python scripting defunct and proceed in reduced functionality mode.
pub fn initialize(
    interpreter: SBCommandInterpreter,
    adapter_dir: &Path,
    console_stream: Option<std::fs::File>,
) -> Result<(Box<PythonInterface>, mpsc::Receiver<EventBody>), Error> {
    let mut command_result = SBCommandReturnObject::new();

    // Import debugger.py into script interpreter's namespace. This also adds our `bin` directory to `sys.path`.
    let init_script = adapter_dir.join("scripts/debugger.py");
    let command = format!("command script import '{}'", init_script.to_str().unwrap());
    interpreter.handle_command(&command, &mut command_result, false);
    if !command_result.succeeded() {
        bail!(format!("{:?}", command_result));
    }
    let (sender, receiver) = mpsc::channel(10);
    let interface = Box::new(PythonInterface {
        initialized: false,
        interpreter: interpreter,
        adapter_dir: adapter_dir.to_owned(),
        event_sender: sender,
        postinit_ptr: NotInitialized,
        handle_message_ptr: NotInitialized,
        compile_code_ptr: NotInitialized,
        shutdown_ptr: NotInitialized,
        evaluate_ptr: NotInitialized,
        evaluate_as_bool_ptr: NotInitialized,
        execute_in_instance_ptr: NotInitialized,
        modules_loaded_ptr: NotInitialized,
        drop_pyobject_ptr: NotInitialized,
    });

    unsafe extern "C" fn init_callback(
        interface: *mut PythonInterface,
        postinit_ptr: *const c_void,
        shutdown_ptr: *const c_void,
        handle_message_ptr: *const c_void,
        compile_code_ptr: *const c_void,
        evaluate_ptr: *const c_void,
        evaluate_as_bool_ptr: *const c_void,
        execute_in_instance_ptr: *const c_void,
        modules_loaded_ptr: *const c_void,
        drop_pyobject_ptr: *const c_void,
    ) {
        (*interface).postinit_ptr = Initialized(mem::transmute(postinit_ptr));
        (*interface).shutdown_ptr = Initialized(mem::transmute(shutdown_ptr));
        (*interface).handle_message_ptr = Initialized(mem::transmute(handle_message_ptr));
        (*interface).compile_code_ptr = Initialized(mem::transmute(compile_code_ptr));
        (*interface).evaluate_ptr = Initialized(mem::transmute(evaluate_ptr));
        (*interface).evaluate_as_bool_ptr = Initialized(mem::transmute(evaluate_as_bool_ptr));
        (*interface).execute_in_instance_ptr = Initialized(mem::transmute(execute_in_instance_ptr));
        (*interface).modules_loaded_ptr = Initialized(mem::transmute(modules_loaded_ptr));
        (*interface).drop_pyobject_ptr = Initialized(mem::transmute(drop_pyobject_ptr));
        (*interface).initialized = true;
    }

    unsafe extern "C" fn send_message_callback(interface: *mut PythonInterface, body_json: *const c_char) {
        let body_json = CStr::from_ptr(body_json).to_str().unwrap().to_string();
        let event = EventBody::_pythonMessage(serde_json::value::RawValue::from_string(body_json).unwrap());
        log_errors!((*interface).event_sender.try_send(event));
    }

    let py_log_level = match log::max_level() {
        log::LevelFilter::Error => 40,
        log::LevelFilter::Warn => 30,
        log::LevelFilter::Info => 20,
        log::LevelFilter::Debug => 10,
        log::LevelFilter::Trace | log::LevelFilter::Off => 0,
    };

    let command = format!(
        "script import codelldb; codelldb.initialize({}, {:p}, {:p}, {:p})",
        py_log_level, init_callback as *const c_void, send_message_callback as *const c_void, interface
    );
    interface.interpreter.handle_command(&command, &mut command_result, false);
    if !command_result.succeeded() {
        bail!(format!("{:?}", command_result));
    }

    // Make sure Python side had called us back.
    if !interface.initialized {
        bail!("Could not initialize Python environment.");
    }

    if let Some(console_stream) = console_stream {
        unsafe {
            (*interface.postinit_ptr)(get_raw_fd(console_stream));
        }
    }

    Ok((interface, receiver))
}

impl PythonInterface {
    pub fn handle_message(&self, body_json: &str) {
        let json_ptr = body_json.as_ptr() as *const c_char;
        let json_size = body_json.len();
        unsafe {
            (*self.handle_message_ptr)(json_ptr, json_size, self.interpreter.debugger());
        }
    }

    // Compiles Python source, returns a code object.
    pub fn compile_code(&self, expr: &str, filename: &str) -> Result<PyObject, Error> {
        let expt_ptr = expr.as_ptr() as *const c_char;
        let expr_size = expr.len();
        let filename_ptr = filename.as_ptr() as *const c_char;
        let filename_size = filename.len();
        let mut result = PyResult::Invalid;
        unsafe {
            (*self.compile_code_ptr)(&mut result, expt_ptr, expr_size, filename_ptr, filename_size);
        }
        match result {
            PyResult::Ok(object) => Ok(PyObject {
                object: object,
                destructor: *self.drop_pyobject_ptr.unwrap(),
            }),
            PyResult::Err(error) => Err(error.into()),
            _ => Err("Evaluation failed".into()),
        }
    }

    // Evaluates compiled code in the specified context.
    pub fn evaluate(
        &self,
        code: &PyObject,
        is_simple_expr: bool,
        context: &SBExecutionContext,
    ) -> Result<SBValue, Error> {
        let mut result = PyResult::Invalid;
        unsafe {
            (*self.evaluate_ptr)(&mut result, code.object, is_simple_expr, context.clone());
        }
        match result {
            PyResult::Ok(value) => Ok(value),
            PyResult::Err(error) => Err(error.into()),
            _ => Err("Evaluation failed".into()),
        }
    }

    // Evaluates compiled code in the specified context, expecting it to yield a boolean.
    pub fn evaluate_as_bool(
        &self,
        code: &PyObject,
        is_simple_expr: bool,
        context: &SBExecutionContext,
    ) -> Result<bool, Error> {
        let mut result = PyResult::Invalid;
        unsafe {
            (*self.evaluate_as_bool_ptr)(&mut result, code.object, is_simple_expr, context.clone());
        }
        match result {
            PyResult::Ok(value) => Ok(value),
            PyResult::Err(error) => Err(error.into()),
            _ => Err("Evaluation failed".into()),
        }
    }

    // Notifies codelldb.py about newly loaded modules.
    pub fn modules_loaded(&self, modules: &mut dyn Iterator<Item = &SBModule>) {
        let modules = modules.cloned().collect::<Vec<SBModule>>();
        unsafe {
            (*self.modules_loaded_ptr)(modules.as_ptr(), modules.len());
        }
    }

    // Execute compiled code in the context of debugger instance's dictionary.
    fn execute_in_instance(&self, code: &PyObject, debugger: &SBDebugger) -> Result<(), Error> {
        let mut result = PyResult::Invalid;
        unsafe {
            (*self.execute_in_instance_ptr)(&mut result, code.object, debugger.clone());
        }
        match result {
            PyResult::Ok(()) => Ok(()),
            PyResult::Err(error) => Err(error.into()),
            _ => Err("Evaluation failed".into()),
        }
    }

    // Load the language support module
    pub fn init_lang_support(&self, langs: &[impl AsRef<str>]) -> Result<(), Error> {
        let mut stmt = String::from("source_languages = [");
        for lang in langs {
            use std::fmt::Write;
            write!(stmt, "'{}',", lang.as_ref())?;
        }
        stmt.push_str("]");
        let code = self.compile_code(&stmt, "<string>")?;
        self.execute_in_instance(&code, &self.interpreter.debugger())?;

        let lang_support = self.adapter_dir.parent().unwrap().join("lang_support");
        let command = format!("command script import '{}'", lang_support.to_str().unwrap());
        let mut command_result = SBCommandReturnObject::new();
        self.interpreter.handle_command(&command, &mut command_result, false);
        if !command_result.succeeded() {
            bail!(format!("{:?}", command_result))
        }
        Ok(())
    }
}

impl Drop for PythonInterface {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                (*self.shutdown_ptr)();
            }
        }
    }
}

#[cfg(unix)]
fn get_raw_fd(stream: std::fs::File) -> usize {
    use std::os::unix::prelude::*;
    stream.into_raw_fd() as usize
}

#[cfg(windows)]
fn get_raw_fd(stream: std::fs::File) -> usize {
    use std::os::windows::prelude::*;
    stream.into_raw_handle() as usize
}

#[test]
fn test_sizeof() {
    // codelldb.py makes assumptions about sizes of these types:
    assert_eq!(mem::size_of::<SBError>(), 8);
    assert_eq!(mem::size_of::<SBDebugger>(), 16);
    assert_eq!(mem::size_of::<SBExecutionContext>(), 16);
    assert_eq!(mem::size_of::<SBValue>(), 16);
    assert_eq!(mem::size_of::<SBModule>(), 16);
    assert_eq!(mem::size_of::<PyObject>(), 16);
}

#[cfg(test)]
lazy_static::lazy_static! {
    static ref DEBUGGER: SBDebugger = {
        use lldb::*;
        std::env::remove_var("PYTHONHOME");
        std::env::remove_var("PYTHONPATH");
        SBDebugger::initialize();
        SBDebugger::create(false)
    };
}

#[test]
fn pypath() {
    use lldb::*;
    let interp = DEBUGGER.command_interpreter();
    let mut result = SBCommandReturnObject::new();
    let status = interp.handle_command("script import sys; print(sys.path)", &mut result, false);
    println!("result = {:?}", result.output());
    assert_eq!(status, ReturnStatus::SuccessFinishNoResult);
}

#[test]
fn evaluate() {
    use lldb::*;
    let interp = DEBUGGER.command_interpreter();
    let adapter_dir = std::env::var("ADAPTER_SOURCE_DIR").unwrap();
    let (python, _) = initialize(interp, Path::new(&adapter_dir), None).unwrap();
    let context = SBExecutionContext::from_target(&DEBUGGER.dummy_target());
    let pycode = python.compile_code("2+2", "<string>").unwrap();
    let result = python.evaluate(&pycode, true, &context);
    println!("result = {:?}", result);
    let value = result.unwrap().value_as_signed(0);
    assert_eq!(value, 4);
}
