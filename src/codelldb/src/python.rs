use crate::prelude::*;

use crate::fsutil::lldb_quoted_string;
use crate::must_initialize::{Initialized, MustInitialize};
use adapter_protocol::{AdapterSettings, EventBody};
use lldb::*;
use serde_derive::*;

use std::collections::HashMap;
use std::ffi::CStr;
use std::mem::{self};
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[repr(C, i32)]
#[allow(dead_code)]
enum PyResult<T> {
    Invalid, // Make dropping a zero-initialized instance safe-ish.
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
        debug!("Dropping object at {:?}", self.object);
        unsafe { (self.destructor)(self.object) }
    }
}

unsafe impl Send for PyObject {}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum PythonEvent {
    DebuggerMessage { output: String, category: String },
    SendDapEvent(EventBody),
}

// Interface through which the rest of CodeLLDB interacts with Python, via C ABI.
// It is intended to be a singleton, since there is only one Python interpreter in LLDB process.
pub struct PythonInterface {
    adapter_dir: PathBuf,
    py: MustInitialize<PythonCalls>,
    session_event_senders: Mutex<HashMap<u64, mpsc::Sender<PythonEvent>>>,
}

struct PythonCalls {
    session_init: unsafe extern "C" fn(debugger: SBDebugger, console_fd: usize) -> bool,
    session_deinit: unsafe extern "C" fn(debugger: SBDebugger) -> bool,
    interrupt: unsafe extern "C" fn(),
    drop_pyobject: unsafe extern "C" fn(obj: *mut c_void),
    handle_message: unsafe extern "C" fn(json: *const c_char, json_len: usize) -> bool,
    compile_code: unsafe extern "C" fn(
        result: *mut PyResult<*mut c_void>,
        expr: *const c_char,
        expr_len: usize,
        filename: *const c_char,
        filename_len: usize,
    ) -> bool,
    evaluate_as_sbvalue: unsafe extern "C" fn(
        result: *mut PyResult<SBValue>,
        code: *mut c_void,
        exec_context: SBExecutionContext,
        eval_context: c_int,
    ) -> bool,
    evaluate_as_bool: unsafe extern "C" fn(
        result: *mut PyResult<bool>,
        pycode: *mut c_void,
        exec_context: SBExecutionContext,
        eval_context: c_int,
    ) -> bool,
}

// Initialize Python interface.
// In order to maintain compatibility with Python 2 (in case we need to load an older liblldb),
// we eschew Python C API, preferring to interact with it via `ctypes` module:
// - We use SBCommandInterpreter to import `adapter/scripts/codelldb` module (which also addes `adapter/scripts`
//   to Python sys.path),
// - We then invoke `codelldb.interface.initialize` function, passing it pointers to various callbacks and data
//   on the Rust side,
// - `initialize()` invokes `init_callback` with pointers to C ABI stubs wrapping Python side callbacks,
//    which are saved and later used to invoke Python code directly, bypassing the slow SBCommandInterpreter API,
// - If any of the above fails, we declare Python scripting defunct and proceed in reduced functionality mode.
pub fn initialize(debugger: &SBDebugger, adapter_dir: &Path) -> Result<Arc<PythonInterface>, Error> {
    let interpreter = debugger.command_interpreter();
    let mut command_result = SBCommandReturnObject::new();

    let script = adapter_dir.join("scripts/codelldb");
    let command = format!("command script import {}", lldb_quoted_string(script.to_str().unwrap()));
    interpreter.handle_command(&command, &mut command_result, false);
    if !command_result.succeeded() {
        bail!(format!("{:?}", command_result));
    }

    let py_log_level = match log::max_level() {
        log::LevelFilter::Error => 40,
        log::LevelFilter::Warn => 30,
        log::LevelFilter::Info => 20,
        log::LevelFilter::Debug => 10,
        log::LevelFilter::Trace | log::LevelFilter::Off => 0,
    };

    // A callback for sending events from Python.
    unsafe extern "C" fn python_event_callback(
        interface: *mut PythonInterface,
        session_id: c_int,
        event: *const c_char,
    ) {
        match serde_json::from_slice::<PythonEvent>(CStr::from_ptr(event).to_bytes()) {
            Ok(event) => (*interface).dispatch_python_event(session_id as u64, event),
            Err(err) => error!("{}", err),
        }
    }

    unsafe extern "C" fn init_callback(
        interface_ptr: *mut PythonInterface,
        pointers: *const *const c_void,
        pointers_len: usize,
    ) {
        if pointers_len != 8 {
            error!("Invalid number of pointers passed to init_callback: {}", pointers_len);
            return;
        }
        let pointers = std::slice::from_raw_parts(pointers, pointers_len);

        let py_calls = PythonCalls {
            session_init: mem::transmute(pointers[0]),
            session_deinit: mem::transmute(pointers[1]),
            interrupt: mem::transmute(pointers[2]),
            drop_pyobject: mem::transmute(pointers[3]),
            handle_message: mem::transmute(pointers[4]),
            compile_code: mem::transmute(pointers[5]),
            evaluate_as_sbvalue: mem::transmute(pointers[6]),
            evaluate_as_bool: mem::transmute(pointers[7]),
        };
        (*interface_ptr).py = Initialized(py_calls);
    }

    let mut interface = Arc::new(PythonInterface {
        adapter_dir: adapter_dir.to_path_buf(),
        py: MustInitialize::NotInitialized,
        session_event_senders: Mutex::new(HashMap::new()),
    });

    let command = format!(
        "script codelldb.interface.initialize({:p}, {:p}, {:p}, {})",
        init_callback as *const c_void,
        Arc::get_mut(&mut interface).unwrap() as *mut _,
        python_event_callback as *const c_void,
        py_log_level,
    );
    interpreter.handle_command(&command, &mut command_result, false);
    if !command_result.succeeded() {
        bail!(format!("{:?}", command_result));
    }
    // Make sure that Python side had called us back.
    if !interface.py.is_initialized() {
        bail!("Could not initialize Python environment.");
    }
    // Leak one reference to keep the interface alive in case python_event_callback() is called unexpectedly.
    mem::forget(interface.clone());

    // Import legacy alias for the codelldb module
    let script = adapter_dir.join("scripts/debugger.py");
    let command = format!("command script import {}", lldb_quoted_string(script.to_str().unwrap()));
    interpreter.handle_command(&command, &mut command_result, false);

    Ok(interface)
}

impl PythonInterface {
    pub fn new_session(
        self: Arc<PythonInterface>,
        debugger: &SBDebugger,
        console_stream: std::fs::File,
    ) -> (PythonSession, mpsc::Receiver<PythonEvent>) {
        let (sender, receiver) = mpsc::channel(100);
        let session = PythonSession {
            interface: self.clone(),
            debugger: debugger.clone(),
        };
        unsafe { (self.py.session_init)(debugger.clone(), get_raw_fd(console_stream)) };
        let mut senders = self.session_event_senders.lock().unwrap();
        senders.insert(debugger.id(), sender);
        (session, receiver)
    }

    // Dispatch Python events to the appropriate DebugSession
    fn dispatch_python_event(&self, session_id: u64, event: PythonEvent) {
        let senders = self.session_event_senders.lock().unwrap();
        if let Some(sender) = senders.get(&session_id) {
            log_errors!(sender.try_send(event));
        } else {
            error!("Received event for non-existent session {}", session_id);
        }
    }
}

// These are per-DebugSession
pub struct PythonSession {
    interface: Arc<PythonInterface>,
    debugger: SBDebugger,
}

#[derive(Debug, Copy, Clone)]
pub enum EvalContext {
    Statement = 0,
    PythonExpression = 1,
    SimpleExpression = 2,
}

impl Drop for PythonSession {
    fn drop(&mut self) {
        unsafe { (self.interface.py.session_deinit)(self.debugger.clone()) };
        let mut senders = self.interface.session_event_senders.lock().unwrap();
        senders.remove(&self.debugger.id());
    }
}

impl PythonSession {
    pub fn handle_message(&self, json: &str) -> bool {
        let json_ptr = json.as_ptr() as *const c_char;
        let json_len = json.len();
        unsafe { (self.interface.py.handle_message)(json_ptr, json_len) }
    }

    // Compiles Python source, returns a code object.
    pub fn compile_code(&self, expr: &str, filename: &str) -> Result<PyObject, Error> {
        debug!("Compiling code: {expr}");
        let expt_ptr = expr.as_ptr() as *const c_char;
        let expr_size = expr.len();
        let filename_ptr = filename.as_ptr() as *const c_char;
        let filename_size = filename.len();
        let mut result = PyResult::Invalid;
        unsafe {
            (self.interface.py.compile_code)(&mut result, expt_ptr, expr_size, filename_ptr, filename_size);
        }
        match result {
            PyResult::Ok(object) => {
                debug!("Created code object at {:?}", object);
                Ok(PyObject {
                    object: object,
                    destructor: self.interface.py.drop_pyobject,
                })
            }
            PyResult::Err(error) => Err(error.into()),
            _ => Err("Evaluation failed".into()),
        }
    }

    // Evaluates compiled code in the specified context.
    pub fn evaluate(
        &self,
        code: &PyObject,
        exec_context: &SBExecutionContext,
        eval_context: EvalContext,
    ) -> Result<SBValue, Error> {
        debug!("Evaluating code object at {:?}", code.object);
        let exec_context = exec_context.clone();
        let eval_context = eval_context as c_int;
        let mut result = PyResult::Invalid;
        unsafe {
            (self.interface.py.evaluate_as_sbvalue)(&mut result, code.object, exec_context, eval_context);
        }
        match result {
            PyResult::Ok(value) => {
                debug!("Evaluation result: {:?}", value);
                Ok(value)
            }
            PyResult::Err(error) => Err(error.into()),
            _ => Err("Evaluation failed".into()),
        }
    }

    // Evaluates compiled code in the specified context, expecting it to yield a boolean.
    pub fn evaluate_as_bool(
        &self,
        code: &PyObject,
        exec_context: &SBExecutionContext,
        eval_context: EvalContext,
    ) -> Result<bool, Error> {
        debug!("Evaluating code object at {:?}", code.object);
        let exec_context = exec_context.clone();
        let eval_context = eval_context as c_int;
        let mut result = PyResult::Invalid;
        unsafe {
            (self.interface.py.evaluate_as_bool)(&mut result, code.object, exec_context, eval_context);
        }
        match result {
            PyResult::Ok(value) => {
                debug!("Evaluation result: {:?}", value);
                Ok(value)
            }
            PyResult::Err(error) => Err(error.into()),
            _ => Err("Evaluation failed".into()),
        }
    }

    // Propagate adapter settings to the Python side.
    pub fn update_adapter_settings(&self, settings: &AdapterSettings) -> Result<(), Error> {
        let settings_json = serde_json::to_string(settings)?;
        let stmt = format!(r#"codelldb.interface.update_adapter_settings("""{settings_json}""", globals())"#);
        let code = self.compile_code(&stmt, "<string>")?;
        let context = SBExecutionContext::from_target(&self.debugger.dummy_target());
        self.evaluate(&code, &context, EvalContext::Statement)?;
        Ok(())
    }

    // Load the language support module
    pub fn init_lang_support(&self) -> Result<(), Error> {
        let lang_support = self.interface.adapter_dir.parent().unwrap().join("lang_support");
        let command = format!("command script import '{}'", lang_support.to_str().unwrap());
        let mut command_result = SBCommandReturnObject::new();
        let interpreter = self.debugger.command_interpreter();
        interpreter.handle_command(&command, &mut command_result, false);
        if !command_result.succeeded() {
            bail!(format!("{:?}", command_result))
        }
        Ok(())
    }

    // Return a callable that sends an interrupt to the Python interpreter.
    pub fn interrupt_sender(&self) -> impl Fn() {
        let interrupt_ptr = self.interface.py.interrupt;
        move || unsafe {
            info!("Sending interrupt to Python interpreter");
            interrupt_ptr();
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

#[test]
fn pypath() {
    use crate::TEST_DEBUGGER;
    use lldb::*;
    let interp = TEST_DEBUGGER.command_interpreter();
    let mut result = SBCommandReturnObject::new();
    let status = interp.handle_command("script import sys; print(sys.path)", &mut result, false);
    println!("result = {:?}", result.output());
    assert_eq!(status, ReturnStatus::SuccessFinishNoResult);
}

#[test]
fn evaluate() {
    use crate::TEST_DEBUGGER;
    use lldb::*;
    let adapter_dir = Path::new(env!("ADAPTER_SOURCE_DIR"));
    let interface = initialize(&TEST_DEBUGGER, adapter_dir).unwrap();
    let (session, _events) = interface.new_session(
        &TEST_DEBUGGER,
        std::fs::File::create(if cfg!(unix) { "/dev/null" } else { "NUL" }).unwrap(),
    );
    let context = SBExecutionContext::from_target(&TEST_DEBUGGER.dummy_target());
    let pycode = session.compile_code("2+2", "<string>").unwrap();
    let result = session.evaluate(&pycode, &context, EvalContext::PythonExpression);
    println!("result = {:?}", result);
    let value = result.unwrap().value_as_signed(0);
    assert_eq!(value, 4);
}

#[test]
fn serialization() {
    serde_json::from_str::<PythonEvent>(
        r#"{"type":"SendDapEvent", "event": "_pythonMessage", "body": {"foo": "bar"} }"#,
    )
    .unwrap();
}
