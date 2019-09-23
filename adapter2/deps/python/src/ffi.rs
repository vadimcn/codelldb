#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]

use std::any::Any;
use std::fmt;
use std::os::raw::{c_char, c_int, c_long, c_longlong, c_void};
use std::ptr;
use std::result::Result;

#[repr(C)]
pub struct PyObject {
    pub ob_refcnt: isize,
    pub ob_type: PyObjectRef,
}

#[repr(u32)]
#[derive(Clone)]
pub enum PyGILState_STATE {
    LOCKED,
    UNLOCKED,
}

pub struct Python {
    gil_state: PyGILState_STATE,
}

macro_rules! py_api {
    { $name:ident ($($arg:ident : $type:ty),* ) -> $res:ty } => {
        pub fn $name(&self, $($arg:$type),*) -> $res {
            #[cfg_attr(windows, link(name="python3", kind="dylib"))]
            extern "C" {
                fn $name ($($arg:$type),*) -> $res;
            }
            unsafe { $name($($arg),*) }
        }
    }
}

macro_rules! py_obj {
    { $name:ident } => {
        pub fn $name() -> PyObjectRef {
            #[cfg_attr(windows, link(name="python3", kind="dylib"))]
            extern "C" {
                static mut $name: PyObject;
            }
            unsafe { PyObjectRef::wrap(&mut $name as *mut PyObject) }
        }
    }
}

type Py_ssize_t = isize;

impl Python {
    py_api!(PyGILState_Ensure() -> PyGILState_STATE);
    py_api!(PyGILState_Release(state: PyGILState_STATE) -> ());

    py_api!(Py_IncRef(obj: PyObjectRef) -> ());
    py_api!(Py_DecRef(obj: PyObjectRef) -> ());

    py_api!(PyImport_ImportModule(name: *const c_char) -> PyObjectResult);

    py_api!(PyObject_IsInstance(inst: PyObjectRef, cls: PyObjectRef) -> c_int);
    py_api!(PyObject_GetAttrString(obj: PyObjectRef, attr_name: *const c_char) -> PyObjectResult);
    py_api!(PyObject_SetAttrString(obj: PyObjectRef, attr_name: *const c_char, val: PyObjectRef) -> PyOkResult);
    py_api!(PyObject_Call(callable: PyObjectRef, args: PyObjectRef, kwargs: Option<PyObjectRef>) -> PyObjectResult);
    py_api!(PyObject_CallObject(callable: PyObjectRef, args: Option<PyObjectRef>) -> PyObjectResult);
    py_api!(PyObject_Str(callable: PyObjectRef) -> PyObjectResult);
    py_api!(PyObject_Repr(callable: PyObjectRef) -> PyObjectResult);

    py_api!(PyCFunction_New(md: *const PyMethodDef, this: Option<PyObjectRef>) -> PyObjectResult);
    py_api!(PyCFunction_NewEx(md: *const PyMethodDef, this: Option<PyObjectRef>, module: Option<PyObjectRef>) -> PyObjectResult);
    py_api!(PyCFunction_Call(func: PyObjectRef, args: PyObjectRef, kwargs: Option<PyObjectRef>) -> PyObjectResult);

    py_api!(PyCapsule_New(pointer: *mut c_void, name: *const c_char, destructor: PyCapsule_Destructor) -> PyObjectResult);
    py_api!(PyCapsule_GetPointer(capsule: PyObjectRef, name: *const c_char) -> *mut c_void);

    py_api!(PyLong_FromLong(val: c_long)-> PyObjectResult);
    py_api!(PyLong_AsSsize_t(pylong: PyObjectRef) -> Py_ssize_t);
    py_api!(PyLong_AsLong(pylong: PyObjectRef) -> c_long);
    py_api!(PyLong_AsLongLong(pylong: PyObjectRef) -> c_longlong);

    py_api!(PyNumber_Long(obj: PyObjectRef) -> PyObjectResult);

    py_api!(PyUnicode_FromStringAndSize(s: *const c_char, len: Py_ssize_t) -> PyObjectResult);

    py_api!(PyBool_FromLong(v: c_long) -> PyObjectResult);

    py_api!(PyTuple_New(len: Py_ssize_t) -> PyObjectResult);
    py_api!(PyTuple_Size(luple: PyObjectRef) -> Py_ssize_t);
    py_api!(PyTuple_SetItem(tuple: PyObjectRef, pos: Py_ssize_t, item: PyObjectOwn) -> PyOkResult);
    py_api!(PyTuple_GetItem(tuple: PyObjectRef, pos: Py_ssize_t) -> PyObjectRef);

    py_api!(PyList_New(len: Py_ssize_t) -> PyObjectResult);
    py_api!(PyList_Append(tup: PyObjectRef, item: PyObjectRef) -> PyOkResult);
    py_api!(PyList_SetItem(tup: PyObjectRef, pos: Py_ssize_t, item: PyObjectOwn) -> PyOkResult);

    py_api!(PySequence_Size(obj: PyObjectRef) -> Py_ssize_t);
    py_api!(PySequence_GetItem(obj: PyObjectRef, pos: Py_ssize_t) -> PyObjectResult);

    py_api!(PyErr_SetObject(err_type: PyObjectRef, value: PyObjectRef) -> ());
    py_api!(PyErr_Occurred() -> PyObjectRef);
    py_api!(PyErr_Fetch(
        ptype: &mut Option<PyObjectOwn>,
        pvalue: &mut Option<PyObjectOwn>,
        ptraceback: &mut Option<PyObjectOwn>
    ) -> ());
    py_api!(PyErr_Restore(err_type: PyObjectOwn, value: PyObjectOwn, traceback: PyObjectOwn) -> ());
    py_api!(PyErr_NormalizeException(
        ptype: &mut PyObjectOwn,
        pvalue: &mut PyObjectOwn,
        ptraceback: &mut PyObjectOwn
    ) -> ());

    py_obj!(PyBool_Type);
    py_obj!(PyLong_Type);
    py_obj!(PyUnicode_Type);
    py_obj!(PyExc_TypeError);
    py_obj!(_Py_TrueStruct);
    py_obj!(_Py_NoneStruct);
}

pub const METH_VARARGS: c_int = 0x0001;
pub const METH_KEYWORDS: c_int = 0x0002;
pub const METH_NOARGS: c_int = 0x0004;
pub const METH_O: c_int = 0x0008;

impl Python {
    pub fn acquire() -> Python {
        let mut py = Python {
            gil_state: PyGILState_STATE::UNLOCKED,
        };
        py.gil_state = py.PyGILState_Ensure();
        py
    }
}

impl Drop for Python {
    fn drop(&mut self) {
        unsafe {
            self.PyGILState_Release(self.gil_state.clone());
        }
    }
}

impl Python {
    // These don't call any Python APIs, so the GIL need not to be held.
    #[inline]
    pub fn Py_TYPE(obj: PyObjectRef) -> PyObjectRef {
        unsafe { obj.0.as_ref().ob_type }
    }

    #[inline]
    pub fn IsInstanceExact(obj: PyObjectRef, ty: PyObjectRef) -> bool {
        Python::Py_TYPE(obj) == ty
    }

    pub fn PyBool_Check(obj: PyObjectRef) -> bool {
        unsafe { Python::IsInstanceExact(obj, Python::PyBool_Type()) }
    }

    pub fn PyLong_Check(obj: PyObjectRef) -> bool {
        unsafe { Python::IsInstanceExact(obj, Python::PyLong_Type()) }
    }

    pub fn PyUnicode_Check(obj: PyObjectRef) -> bool {
        unsafe { Python::IsInstanceExact(obj, Python::PyUnicode_Type()) }
    }

    pub fn pybool_as_bool(&self, value: PyObjectRef) -> bool {
        unsafe { value == Python::_Py_TrueStruct() }
    }

    pub fn pystring_as_str(&self, pystr: &PyObjectOwn) -> &str {
        #[cfg_attr(windows, link(name = "python3", kind = "dylib"))]
        extern "C" {
            fn PyArg_Parse(obj: PyObjectRef, format: *const c_char, ...) -> c_int;
        }
        let py = self;
        let mut ptr = ptr::null();
        let mut size = 0;
        unsafe {
            PyArg_Parse(pystr.get(), "s#\0".as_ptr() as *const c_char, &ptr, &size);
            let bytes = std::slice::from_raw_parts(ptr as *const u8, size as usize);
            std::str::from_utf8(bytes).unwrap()
        }
    }

    pub fn pystring_from_str(&self, s: &str) -> PyObjectResult {
        self.PyUnicode_FromStringAndSize(s.as_ptr() as *const c_char, s.len() as isize)
    }

    pub fn make_pytuple(&self, mut objs: Vec<PyObjectOwn>) -> Result<PyObjectOwn, PyErr> {
        let py = self;
        let tuple = py.PyTuple_New(objs.len() as isize)?;
        for (i, obj) in objs.drain(..).enumerate() {
            py.PyTuple_SetItem(tuple.get(), i as isize, obj)?;
        }
        Ok(tuple)
    }

    pub fn extract_string(&self, obj: PyObjectRef) -> Result<String, PyErr> {
        if !Python::PyUnicode_Check(obj) {
            Err(PyErr::raise(Python::PyExc_TypeError(), "expected a string"))
        } else {
            Ok(self.pystring_as_str(&obj.into()).to_string())
        }
    }

    pub fn extract_i32(&self, obj: PyObjectRef) -> Result<i32, PyErr> {
        if !Python::PyLong_Check(obj) {
            Err(PyErr::raise(Python::PyExc_TypeError(), "expected an int"))
        } else {
            Ok(self.PyLong_AsLong(obj) as i32)
        }
    }

    pub fn extract_bool(&self, obj: PyObjectRef) -> Result<bool, PyErr> {
        if !Python::PyBool_Check(obj) {
            Err(PyErr::raise(Python::PyExc_TypeError(), "expected a boolean"))
        } else {
            Ok(self.pybool_as_bool(obj))
        }
    }

    pub fn parse_tuple(&self, tuple: PyObjectRef, args: &mut [&mut dyn Any], required: usize) -> Result<(), PyErr> {
        let py = self;
        let num_values = py.PyTuple_Size(tuple) as usize;
        for (i, arg) in args.iter_mut().enumerate() {
            if i >= num_values {
                if i < required {
                    return Err(PyErr::raise(
                        Python::PyExc_TypeError(),
                        format!("expected {} arguments, got {}.", required, num_values),
                    ));
                }
                break;
            }

            let value = py.PyTuple_GetItem(tuple, i as isize);

            if let Some(s) = arg.downcast_mut::<String>() {
                *s = py.extract_string(value)?;
            } else if let Some(s) = arg.downcast_mut::<Option<String>>() {
                *s = Some(py.extract_string(value)?);
            } else if let Some(i) = arg.downcast_mut::<i32>() {
                *i = py.extract_i32(value)?;
            } else if let Some(i) = arg.downcast_mut::<Option<i32>>() {
                *i = Some(py.extract_i32(value)?);
            } else if let Some(b) = arg.downcast_mut::<bool>() {
                *b = py.extract_bool(value)?;
            } else if let Some(b) = arg.downcast_mut::<Option<bool>>() {
                *b = Some(py.extract_bool(value)?);
            } else {
                panic!("Unsupported data type.");
            }
        }
        Ok(())
    }
}

// Non-owning reference to PyObject.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PyObjectRef(ptr::NonNull<PyObject>);
impl PyObjectRef {
    pub fn wrap(ptr: *mut PyObject) -> PyObjectRef {
        PyObjectRef(ptr::NonNull::new(ptr).unwrap())
    }
}
unsafe impl Send for PyObjectRef {}
impl From<*mut PyObject> for PyObjectRef {
    fn from(ptr: *mut PyObject) -> Self {
        Self::wrap(ptr)
    }
}
impl fmt::Debug for PyObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let py = Python::acquire();
        let pystr = py.PyObject_Repr(*self).into_result().map_err(|_| fmt::Error)?;
        f.write_str(py.pystring_as_str(&pystr))
    }
}
impl fmt::Display for PyObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let py = Python::acquire();
        let pystr = py.PyObject_Str(*self).into_result().map_err(|_| fmt::Error)?;
        f.write_str(py.pystring_as_str(&pystr))
    }
}

// Owning reference to PyObject.
#[repr(transparent)]
pub struct PyObjectOwn(ptr::NonNull<PyObject>);
impl PyObjectOwn {
    pub fn wrap(ptr: *mut PyObject) -> PyObjectOwn {
        PyObjectOwn(ptr::NonNull::new(ptr).unwrap())
    }
    pub fn get(&self) -> PyObjectRef {
        PyObjectRef(self.0)
    }
}
unsafe impl Send for PyObjectOwn {}
impl Drop for PyObjectOwn {
    fn drop(&mut self) {
        let py = Python::acquire();
        py.Py_DecRef(self.get());
    }
}
impl Clone for PyObjectOwn {
    fn clone(&self) -> Self {
        let py = Python::acquire();
        py.Py_IncRef(self.get());
        PyObjectOwn(self.0)
    }
}
impl From<PyObjectRef> for PyObjectOwn {
    fn from(r: PyObjectRef) -> Self {
        let py = Python::acquire();
        py.Py_IncRef(r);
        PyObjectOwn(r.0)
    }
}
impl fmt::Debug for PyObjectOwn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.get().fmt(f)
    }
}
impl fmt::Display for PyObjectOwn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.get().fmt(f)
    }
}

#[repr(C)]
pub struct PyObjectResult(Option<PyObjectOwn>);
impl std::ops::Try for PyObjectResult {
    type Ok = PyObjectOwn;
    type Error = PyErr;

    fn into_result(self) -> Result<Self::Ok, Self::Error> {
        match self.0 {
            Some(obj) => Ok(obj),
            None => Err(PyErr::fetch()),
        }
    }
    fn from_ok(obj: PyObjectOwn) -> Self {
        PyObjectResult(Some(obj))
    }
    fn from_error(err: PyErr) -> Self {
        let py = Python::acquire();
        py.PyErr_Restore(err.err_type, err.value, err.traceback);
        PyObjectResult(None)
    }
}
impl PyObjectResult {
    pub fn into_result(self) -> Result<PyObjectOwn, PyErr> {
        <Self as std::ops::Try>::into_result(self)
    }
    pub fn from_result(result: Result<PyObjectOwn, PyErr>) -> Self {
        match result {
            Ok(obj) => <Self as std::ops::Try>::from_ok(obj),
            Err(err) => <Self as std::ops::Try>::from_error(err),
        }
    }
    pub fn unwrap(self) -> PyObjectOwn {
        match self.0 {
            Some(obj) => obj,
            None => panic!("called `unwrap()` on null PyObjectResult"),
        }
    }
}

// For c_int return values, where 0 is Ok and -1 is Err.
#[repr(C)]
pub struct PyOkResult(c_int);
impl std::ops::Try for PyOkResult {
    type Ok = ();
    type Error = PyErr;

    fn into_result(self) -> Result<Self::Ok, Self::Error> {
        match self.0 {
            0 => Ok(()),
            _ => Err(PyErr::fetch()),
        }
    }
    fn from_ok(_: Self::Ok) -> Self {
        PyOkResult(0)
    }
    fn from_error(_: Self::Error) -> Self {
        PyOkResult(-1)
    }
}
impl PyOkResult {
    pub fn into_result(self) -> Result<(), PyErr> {
        <Self as std::ops::Try>::into_result(self)
    }
    pub fn unwrap(self) -> () {
        self.into_result().unwrap();
    }
}

pub struct PyErr {
    pub err_type: PyObjectOwn,
    pub value: PyObjectOwn,
    pub traceback: PyObjectOwn,
}

impl PyErr {
    pub fn raise(err_type: PyObjectRef, msg: impl AsRef<str>) -> PyErr {
        let py = Python::acquire();
        let value = py.pystring_from_str(msg.as_ref()).unwrap();
        unsafe { py.PyErr_SetObject(err_type, value.get()) };
        PyErr::fetch()
    }

    pub fn fetch() -> Self {
        let py = Python::acquire();
        let mut err_type = None;
        let mut value = None;
        let mut traceback = None;
        py.PyErr_Fetch(&mut err_type, &mut value, &mut traceback);
        PyErr {
            err_type: err_type.unwrap(),
            value: value.unwrap(),
            traceback: traceback.unwrap(),
        }
    }

    pub fn normalize(&mut self) {
        let py = Python::acquire();
        py.PyErr_NormalizeException(&mut self.err_type, &mut self.value, &mut self.traceback);
    }
}
impl fmt::Debug for PyErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let py = Python::acquire();
        let pystr = py.PyObject_Str(self.value.get()).into_result().map_err(|_| fmt::Error)?;
        write!(f, "{}", py.pystring_as_str(&pystr))
    }
}
impl fmt::Display for PyErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for PyErr {}
impl Into<String> for PyErr {
    fn into(self) -> String {
        format!("{:?}", self)
    }
}

pub type PyCFunction = extern "C" fn(PyObjectRef, PyObjectRef) -> PyObjectResult;
pub type PyCapsule_Destructor = extern "C" fn(PyObjectRef);

#[repr(C)]
pub struct PyMethodDef {
    pub ml_name: *const c_char,
    pub ml_meth: PyCFunction,
    pub ml_flags: c_int,
    pub ml_doc: *const c_char,
}

#[test]
fn check_sizes() {
    use std::mem::size_of;
    let ptr_size = size_of::<*mut PyObject>();
    assert_eq!(size_of::<PyObjectRef>(), ptr_size);
    assert_eq!(size_of::<Option<PyObjectRef>>(), ptr_size);
    assert_eq!(size_of::<PyObjectOwn>(), ptr_size);
    assert_eq!(size_of::<Option<PyObjectOwn>>(), ptr_size);
    assert_eq!(size_of::<PyObjectResult>(), ptr_size);
    assert_eq!(size_of::<PyOkResult>(), size_of::<c_int>());
}

#[test]
fn check_repr() {
    unsafe {
        let x: Option<PyObjectRef> = None;
        assert_eq!(*(&x as *const _ as *const usize), 0);
        let x: Option<PyObjectOwn> = None;
        assert_eq!(*(&x as *const _ as *const usize), 0);
    }
}
#[test]
fn parse_tuple() {
    unsafe {
        Py_InitializeEx(0);
        let py = Python::acquire();

        let t = py
            .make_pytuple(vec![
                py.pystring_from_str("String").unwrap(),
                py.PyBool_FromLong(1).unwrap(),
                py.PyInt_FromLong(42).unwrap(),
            ])
            .unwrap();

        let mut s = String::new();
        let mut b = false;
        let mut i = 0;
        py.parse_tuple(t.get(), &mut [&mut s, &mut b, &mut i], 3).unwrap();

        let mut s: Option<String> = None;
        let mut b: Option<bool> = None;
        let mut i: Option<i32> = None;;
        py.parse_tuple(t.get(), &mut [&mut s, &mut b, &mut i], 3).unwrap();
    }
}
