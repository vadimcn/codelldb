use log::{debug, error, info};
use std::cell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::env;
use std::fmt::Write;
use std::os::raw::{c_char, c_int, c_void};
use std::slice;
use std::str;

use cpython::*;
use lldb::*;

use crate::debug_protocol::{DisplayHtmlEventBody, EventBody};
use crate::error::Error;

#[derive(Debug)]
pub enum PythonValue {
    SBValue(SBValue),
    Int(i64),
    Bool(bool),
    String(String),
    Object(String),
}

pub struct PythonInterface {
    interpreter: SBCommandInterpreter,
    pymod_lldb: PyModule,
    pymod_codelldb: PyModule,
    pymod_traceback: PyModule,
    pyty_sbexec_context: PyType,
    pyty_sbmodule: PyType,
    pyty_sbvalue: PyType,
    pyfn_evaluate_in_frame: PyObject,
    pyfn_modules_loaded: PyObject,
}

impl PythonInterface {
    pub fn new(
        interpreter: SBCommandInterpreter, send_event: Box<Fn(EventBody) + Send>,
    ) -> Result<Box<PythonInterface>, Error> {
        let current_exe = env::current_exe()?;
        let mut command_result = SBCommandReturnObject::new();

        // Import debugger.py into script interpreter's namespace.
        // This also adds our bin directory to sys.path, so we can import the rest of our modules below.
        let init_script = current_exe.with_file_name("debugger.py");
        let command = format!("command script import '{}'", init_script.to_str()?);
        interpreter.handle_command(&command, &mut command_result, false);
        info!("{:?}", command_result);

        let rust_formatters = current_exe.with_file_name("rust.py");
        let command = format!("command script import '{}'", rust_formatters.to_str()?);
        interpreter.handle_command(&command, &mut command_result, false);
        info!("{:?}", command_result);

        // Cache some objects
        let gil = Python::acquire_gil();
        let py = gil.python();

        let pymod_lldb = py.import("lldb").unwrap();
        let pyty_sbvalue = PyType::downcast_from(py, pymod_lldb.get(py, "SBValue").unwrap()).unwrap();
        let pyty_sbmodule = PyType::downcast_from(py, pymod_lldb.get(py, "SBModule").unwrap()).unwrap();
        let pyty_sbexec_context = PyType::downcast_from(py, pymod_lldb.get(py, "SBExecutionContext").unwrap()).unwrap();

        let pymod_codelldb = py.import("codelldb").unwrap();
        let pyfn_evaluate_in_frame = pymod_codelldb.get(py, "evaluate_in_frame").unwrap();
        let pyfn_modules_loaded = pymod_codelldb.get(py, "modules_loaded").unwrap();

        let pymod_traceback = py.import("traceback").unwrap();

        let callback = RustClosure::new(py, move |py: Python, args, kwargs| {
            py_argparse!(py, None, args, kwargs, (
                    html: String,
                    title: Option<String> = None,
                    position: Option<i32> = None,
                    reveal: bool = false
            ) {
                send_event(EventBody::displayHtml(DisplayHtmlEventBody {
                    html, title, position, reveal
                }));
                Ok(py.None())
            })
        });
        pymod_codelldb.as_object().setattr(py, "display_html", callback.unwrap());

        let mut py_interface = Box::new(PythonInterface {
            interpreter,
            pymod_lldb,
            pymod_codelldb,
            pymod_traceback,
            pyty_sbexec_context,
            pyty_sbmodule,
            pyty_sbvalue,
            pyfn_evaluate_in_frame,
            pyfn_modules_loaded,
        });
        Ok(py_interface)
    }

    pub fn evaluate(&self, expr: &str, is_simple_expr: bool, context: &SBExecutionContext) -> Result<SBValue, String> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let pysb_exec_context =
            unsafe { into_swig_wrapper::<SBExecutionContext>(py, context.clone(), &self.pyty_sbexec_context) };
        let result = self.pyfn_evaluate_in_frame.call(py, (expr, is_simple_expr, pysb_exec_context), None);
        let target = context.target().unwrap();
        let result = self.to_sbvalue(py, &target, result);
        debug!("evaluate {} -> {:?}", expr, result);
        result
    }

    pub fn evaluate_as_bool(
        &self, expr: &str, is_simple_expr: bool, context: &SBExecutionContext,
    ) -> Result<bool, String> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let pysb_exec_context =
            unsafe { into_swig_wrapper::<SBExecutionContext>(py, context.clone(), &self.pyty_sbexec_context) };
        let result = self.pyfn_evaluate_in_frame.call(py, (expr, is_simple_expr, pysb_exec_context), None);
        let result = match result {
            Ok(value) => Ok(value.is_true(py).unwrap()),
            Err(pyerr) => Err(self.format_exception(py, pyerr)),
        };
        debug!("evaluate_as_bool {} -> {:?}", expr, result);
        result
    }

    pub fn modules_loaded(&self, modules: &mut Iterator<Item = &SBModule>) {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let list = PyList::new(py, &[]);
        for module in modules {
            let pysbmodule = unsafe { into_swig_wrapper::<SBModule>(py, module.clone(), &self.pyty_sbmodule) };
            list.insert_item(py, list.len(py), pysbmodule);
        }
        self.pyfn_modules_loaded.call(py, (list,), None).unwrap();
    }

    fn to_sbvalue(&self, py: Python, target: &SBTarget, result: PyResult<PyObject>) -> Result<SBValue, String> {
        match result {
            Ok(value) => {
                if self.pyty_sbvalue.is_instance(py, &value) {
                    let sbvalue = unsafe { from_swig_wrapper::<SBValue>(py, &value) };
                    Ok(sbvalue)
                } else if PyBool::type_object(py).is_instance(py, &value) {
                    let value = bool::extract(py, &value).unwrap();
                    Ok(sbvalue_from_bool(value, target))
                } else if PyInt::type_object(py).is_instance(py, &value) {
                    let value = i64::extract(py, &value).unwrap();
                    Ok(sbvalue_from_i64(value, target))
                } else if PyLong::type_object(py).is_instance(py, &value) {
                    let value = i64::extract(py, &value).unwrap();
                    Ok(sbvalue_from_i64(value, target))
                } else if PyString::type_object(py).is_instance(py, &value) {
                    let value = String::extract(py, &value).unwrap();
                    Ok(sbvalue_from_str(&value, target))
                } else {
                    let value = value.to_string();
                    Ok(sbvalue_from_str(&value, target))
                }
            }
            Err(pyerr) => Err(self.format_exception(py, pyerr)),
        }
    }

    fn format_exception(&self, py: Python, mut err: PyErr) -> String {
        err.normalize(py);
        match self.pymod_traceback.call(py, "format_exception", (&err.ptype, &err.pvalue, &err.ptraceback), None) {
            Ok(tb) => {
                let lines = Vec::<String>::extract(py, &tb).unwrap();
                lines.concat()
            }
            Err(_) => format!("Could not format exception: {:?}", err),
        }
    }
}

// Creates a SWIG wrapper containing native SB object.
// `pytype` is the Python type object of the wrapper.
// Obviously, `SBT` and `pytype` must match, hence `unsafe`.
unsafe fn into_swig_wrapper<SBT>(py: Python, obj: SBT, pytype: &PyType) -> PyObject {
    // SWIG does not provide an API for creating Python wrapper from a native object, so we have to employ a bit of trickery:
    // First, we call SB wrapper's constructor on the Python side, which creates an instance wrapping dummy native SB object,
    let pysb = pytype.call(py, NoArgs, None).unwrap();
    // then, we retrieve a pointer to the native object via wrapper's `this` attribute,
    let this = pysb.getattr(py, "this").unwrap().extract::<usize>(py).unwrap();
    // finally, we replace it with the actual SB object we wanted to wrap.
    std::ptr::replace(this as *mut SBT, obj);
    pysb
}

// Extracts native SB object from a SWIG wrapper.
unsafe fn from_swig_wrapper<SBT>(py: Python, pyobj: &PyObject) -> SBT
where
    SBT: Clone,
{
    let this = pyobj.getattr(py, "this").unwrap().extract::<usize>(py).unwrap();
    let mut sb = this as *const SBT;
    (*sb).clone()
}

fn sbvalue_from_bool(value: bool, target: &SBTarget) -> SBValue {
    let ty = target.get_basic_type(BasicType::Bool);
    let slice = [value as u8, 0, 0, 0, 0, 0, 0, 0]; // 8 bytes ought to be enough to hold a bool on any platform
    let data = SBData::borrow_bytes(&slice, ByteOrder::Little, 8);
    target.create_value_from_data("result", &data, &ty)
}

fn sbvalue_from_i64(value: i64, target: &SBTarget) -> SBValue {
    let ty = target.get_basic_type(BasicType::LongLong);
    let bytes = value.to_le_bytes();
    let data = SBData::borrow_bytes(&bytes, ByteOrder::Little, 8);
    target.create_value_from_data("result", &data, &ty)
}

fn sbvalue_from_str(value: &str, target: &SBTarget) -> SBValue {
    let ty = target.get_basic_type(BasicType::Char).array_type(value.len() as u64);
    let bytes = value.as_bytes();
    let data = SBData::borrow_bytes(bytes, ByteOrder::Little, 8);
    target.create_value_from_data("result", &data, &ty)
}

// Python wrapper for Rust closures
py_class!(class RustClosure |py| {
    data closure: cell::RefCell<Box<FnMut(Python, &PyTuple, Option<&PyDict>) -> PyResult<PyObject> + Send>>;

    def __call__(&self, *args, **kwargs) -> PyResult<PyObject> {
        use std::ops::DerefMut;
        let mut mut_ref = self.closure(py).borrow_mut();
        mut_ref.deref_mut()(py, &args, kwargs)
    }
});

impl RustClosure {
    fn new<F>(py: Python, closure: F) -> PyResult<RustClosure>
    where
        F: FnMut(Python, &PyTuple, Option<&PyDict>) -> PyResult<PyObject> + Send + 'static,
    {
        let closure = cell::RefCell::new(Box::new(closure));
        RustClosure::create_instance(py, closure)
    }
}
