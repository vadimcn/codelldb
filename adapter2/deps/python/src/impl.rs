#![feature(try_trait)]

use codelldb_python::*;
use lldb::*;
use log::{self, error};
use std::env;
use std::fmt::Write;
use std::os::raw::{c_char, c_long, c_void};
use std::str;

mod ffi;
pub use ffi::PyErr;
use ffi::*;

macro_rules! cstr {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const c_char
    };
}

#[derive(Debug)]
pub enum PythonValue {
    SBValue(SBValue),
    Int(i64),
    Bool(bool),
    String(String),
    Object(String),
}

pub struct PythonInterfaceImpl {
    pyty_sbexec_context: PyObjectOwn,
    pyty_sbmodule: PyObjectOwn,
    pyty_sbvalue: PyObjectOwn,
    pyfn_evaluate_in_frame: PyObjectOwn,
    pyfn_modules_loaded: PyObjectOwn,
    pyfn_format_exception: PyObjectOwn,
}

#[no_mangle]
pub fn entry() -> Result<(), Error> {
    env_logger::Builder::from_default_env().init();
    Ok(())
}

#[no_mangle]
pub fn new_session(
    interpreter: SBCommandInterpreter,
    event_sink: Box<dyn EventSink + Send>,
) -> Result<Box<dyn PythonInterface>, Error> {
    let current_exe = env::current_exe()?;
    let mut command_result = SBCommandReturnObject::new();

    // Import debugger.py into script interpreter's namespace.
    // This also adds our bin directory to sys.path, so we can import the rest of the modules below.
    let init_script = current_exe.with_file_name("debugger.py");
    let command = format!("command script import '{}'", init_script.to_str().unwrap()); /*####*/
    interpreter.handle_command(&command, &mut command_result, false);
    if !command_result.succeeded() {
        return Err(format!("{:?}", command_result).into());
    }

    // Init python logging
    let py_log_level = match log::max_level() {
        log::LevelFilter::Error => 40,
        log::LevelFilter::Warn => 30,
        log::LevelFilter::Info => 20,
        log::LevelFilter::Debug => 10,
        log::LevelFilter::Trace | log::LevelFilter::Off => 0,
    };

    let py = Python::acquire();
    let pymod_codelldb = py.PyImport_ImportModule(cstr!("codelldb"))?;
    let set_log_level = py.PyObject_GetAttrString(pymod_codelldb.get(), cstr!("set_log_level"))?;
    let py_log_level = py.PyLong_FromLong(py_log_level)?;
    let args = py.make_pytuple(vec![py_log_level])?;
    py.PyObject_CallObject(set_log_level.get(), Some(args.get()))?;
    drop(py);

    let rust_formatters = current_exe.with_file_name("rust.py");
    let command = format!("command script import '{}'", rust_formatters.to_str().unwrap()); /*###*/
    interpreter.handle_command(&command, &mut command_result, false);
    if !command_result.succeeded() {
        error!("{:?}", command_result); // But carry on - Rust formatters are not critical to have.
    }

    // Cache some objects
    let py = Python::acquire();

    let pymod_lldb = py.PyImport_ImportModule(cstr!("lldb"))?;
    let pyty_sbvalue = py.PyObject_GetAttrString(pymod_lldb.get(), cstr!("SBValue"))?;
    let pyty_sbmodule = py.PyObject_GetAttrString(pymod_lldb.get(), cstr!("SBModule"))?;
    let pyty_sbexec_context = py.PyObject_GetAttrString(pymod_lldb.get(), cstr!("SBExecutionContext"))?;

    let pyfn_evaluate_in_frame = py.PyObject_GetAttrString(pymod_codelldb.get(), cstr!("evaluate_in_frame"))?;
    let pyfn_modules_loaded = py.PyObject_GetAttrString(pymod_codelldb.get(), cstr!("modules_loaded"))?;

    let pymod_traceback = py.PyImport_ImportModule(cstr!("traceback"))?;
    let pyfn_format_exception = py.PyObject_GetAttrString(pymod_traceback.get(), cstr!("format_exception"))?;

    let callback = wrap_callable(move |args| {
        let py = Python::acquire();
        let mut html = String::new();
        let mut title = None;
        let mut position = None;
        let mut reveal = false;
        py.parse_tuple(args, &mut [&mut html, &mut title, &mut position, &mut reveal], 1)?;
        event_sink.display_html(html, title, position, reveal);
        Ok(Python::_Py_NoneStruct().into())
    })?;
    py.PyObject_SetAttrString(pymod_codelldb.get(), cstr!("display_html"), callback.get());

    let py_interface = Box::new(PythonInterfaceImpl {
        pyty_sbexec_context,
        pyty_sbmodule,
        pyty_sbvalue,
        pyfn_evaluate_in_frame,
        pyfn_modules_loaded,
        pyfn_format_exception,
    });
    Ok(py_interface)
}

impl PythonInterface for PythonInterfaceImpl {
    fn evaluate(&self, expr: &str, is_simple_expr: bool, context: &SBExecutionContext) -> Result<SBValue, String> {
        let py = Python::acquire();
        let target = context.target().unwrap();
        let result = self
            .evaluate_core(&py, expr, is_simple_expr, context)
            .and_then(|value| self.to_sbvalue(&py, &target, value.get()))
            .map_err(|err| self.format_exception(&py, err));
        result
    }

    fn evaluate_as_bool(&self, expr: &str, is_simple_expr: bool, context: &SBExecutionContext) -> Result<bool, String> {
        let py = Python::acquire();
        let result = self
            .evaluate_core(&py, expr, is_simple_expr, context)
            .map(|value| py.pybool_as_bool(value.get()))
            .map_err(|err| self.format_exception(&py, err));
        result
    }

    fn modules_loaded(&self, modules: &mut dyn Iterator<Item = &SBModule>) {
        let py = Python::acquire();
        let result = || -> Result<(), PyErr> {
            let list = py.PyList_New(0)?;
            for module in modules {
                let pysbmodule =
                    unsafe { into_swig_wrapper::<SBModule>(&py, module.clone(), self.pyty_sbmodule.get()) };
                py.PyList_Append(list.get(), pysbmodule.get())?;
            }
            let args = py.make_pytuple(vec![list])?;
            py.PyObject_CallObject(self.pyfn_modules_loaded.get(), Some(args.get()))?;
            Ok(())
        }();
        if let Err(err) = result {
            error!("modules_loaded: {}", self.format_exception(&py, err));
        }
    }
}

impl PythonInterfaceImpl {
    fn evaluate_core(
        &self,
        py: &Python,
        expr: &str,
        is_simple_expr: bool,
        context: &SBExecutionContext,
    ) -> Result<PyObjectOwn, PyErr> {
        let pysb_exec_context =
            unsafe { into_swig_wrapper::<SBExecutionContext>(&py, context.clone(), self.pyty_sbexec_context.get()) };
        let args = py.make_pytuple(vec![
            py.pystring_from_str(expr)?,
            py.PyBool_FromLong(is_simple_expr as c_long)?,
            pysb_exec_context,
        ])?;
        let value = py.PyObject_CallObject(self.pyfn_evaluate_in_frame.get(), Some(args.get()))?;
        Ok(value)
    }

    fn to_sbvalue(&self, py: &Python, target: &SBTarget, value: PyObjectRef) -> Result<SBValue, PyErr> {
        if py.PyObject_IsInstance(value, self.pyty_sbvalue.get()) != 0 {
            let sbvalue = unsafe { from_swig_wrapper::<SBValue>(&py, value) };
            Ok(sbvalue)
        } else if Python::PyBool_Check(value) {
            let b = py.pybool_as_bool(value);
            Ok(sbvalue_from_bool(b, target))
        } else if Python::PyLong_Check(value) {
            let ll = py.PyLong_AsLongLong(value);
            Ok(sbvalue_from_i64(ll as i64, target))
        } else if Python::PyUnicode_Check(value) {
            let value = value.into();
            let s = py.pystring_as_str(&value);
            Ok(sbvalue_from_str(s, target))
        } else {
            let val_str = py.PyObject_Str(value).unwrap();
            let s = py.pystring_as_str(&val_str);
            Ok(sbvalue_from_str(s, target))
        }
    }

    fn format_exception(&self, py: &Python, mut err: PyErr) -> String {
        let result = || -> Result<String, PyErr> {
            err.normalize();
            let args = py.make_pytuple(vec![err.err_type.clone(), err.value.clone(), err.traceback.clone()])?;
            let lines = py.PyObject_CallObject(self.pyfn_format_exception.get(), Some(args.get()))?;

            let mut s = String::new();
            for i in 0..(py.PySequence_Size(lines.get())) {
                let line = py.PySequence_GetItem(lines.get(), i)?;
                write!(s, "{}", py.pystring_as_str(&line)).unwrap();
            }
            Ok(s)
        }();
        match result {
            Ok(s) => s,
            Err(_) => format!("Could not format exception: {:?}", err),
        }
    }
}

// Creates a SWIG wrapper containing native SB object.
// `pytype` is the Python type object of the wrapper.
// Obviously, `SBT` and `pytype` must match, hence `unsafe`.
unsafe fn into_swig_wrapper<SBT>(py: &Python, obj: SBT, pytype: PyObjectRef) -> PyObjectOwn {
    // SWIG does not provide an API for creating Python wrapper from a native object, so we have to employ a bit of trickery:
    // First, we call SB wrapper's constructor on the Python side, which creates an instance wrapping dummy native SB object,
    let pysb = py.PyObject_CallObject(pytype, None).unwrap();
    // then, we retrieve a pointer to the native object via wrapper's `this` attribute,
    let this = py.PyObject_GetAttrString(pysb.get(), cstr!("this")).unwrap();
    let this = py.PyNumber_Long(this.get()).unwrap();
    let this = py.PyLong_AsSsize_t(this.get());
    // finally, we replace it with the actual SB object we wanted to wrap.
    std::ptr::replace(this as *mut SBT, obj);
    pysb
}

// Extracts native SB object from a SWIG wrapper.
unsafe fn from_swig_wrapper<SBT>(py: &Python, pyobj: PyObjectRef) -> SBT
where
    SBT: Clone,
{
    let this = py.PyObject_GetAttrString(pyobj, cstr!("this")).unwrap();
    let this = py.PyNumber_Long(this.get()).unwrap();
    let this = py.PyLong_AsSsize_t(this.get());
    let sb = this as *const SBT;
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

fn wrap_callable<F>(closure: F) -> Result<PyObjectOwn, PyErr>
where
    F: FnMut(PyObjectRef) -> Result<PyObjectOwn, PyErr> + Send + 'static,
{
    unsafe {
        let py = Python::acquire();

        extern "C" fn destructor<Data>(capsule: PyObjectRef) {
            let py = Python::acquire();
            let ptr = py.PyCapsule_GetPointer(capsule, cstr!("RustClosure"));
            unsafe { Box::from_raw(ptr as *mut Data) };
        }

        extern "C" fn trampoline<F>(capsule: PyObjectRef, args: PyObjectRef) -> PyObjectResult
        where
            F: FnMut(PyObjectRef) -> Result<PyObjectOwn, PyErr>,
        {
            let py = Python::acquire();
            let ptr = py.PyCapsule_GetPointer(capsule, cstr!("RustClosure"));
            let data: &mut _ = unsafe { &mut *(ptr as *mut (F, PyMethodDef)) };
            PyObjectResult::from_result((data.0)(args))
        }

        let md = PyMethodDef {
            ml_name: cstr!("RustClosure"),
            ml_meth: trampoline::<F>,
            ml_flags: METH_VARARGS,
            ml_doc: cstr!("Rust closure"),
        };

        let data = (closure, md);
        let ptr = Box::into_raw(Box::new(data));
        let capsule = py.PyCapsule_New(ptr as *mut c_void, cstr!("RustClosure"), destructor::<(F, PyMethodDef)>)?;

        let func = py.PyCFunction_New(&(*ptr).1, Some(capsule.get()))?;
        Ok(func)
    }
}
