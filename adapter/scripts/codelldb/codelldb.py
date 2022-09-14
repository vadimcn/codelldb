import __main__
import sys
import os
import lldb
import logging
import traceback
import ctypes
from ctypes import (CFUNCTYPE, POINTER, py_object, sizeof, byref, memmove,
                    c_bool, c_char, c_char_p, c_int, c_int64, c_double, c_size_t, c_void_p)
from .value import Value

log = logging.getLogger('codelldb')

try: from typing import Tuple
except Exception: pass

#============================================================================================

# 8 bytes
class SBError(ctypes.Structure):
    _fields_ = [('_opaque', c_int64)]
    swig_type = lldb.SBError

# 16 bytes
class SBDebugger(ctypes.Structure):
    _fields_ = [('_opaque', c_int64 * 2)]
    swig_type = lldb.SBDebugger

class SBExecutionContext(ctypes.Structure):
    _fields_ = [('_opaque', c_int64 * 2)]
    swig_type = lldb.SBExecutionContext

class SBValue(ctypes.Structure):
    _fields_ = [('_opaque', c_int64 * 2)]
    swig_type = lldb.SBValue

class SBModule(ctypes.Structure):
    _fields_ = [('_opaque', c_int64 * 2)]
    swig_type = lldb.SBModule

# Convert one of the above raw SB objects (https://lldb.llvm.org/design/sbapi.html) into a SWIG wrapper.
# We rely on the fact that SB objects consist of a single shared_ptr, which can be moved around freely.
# There are 3 memory regions in play:
#  [1:SWIG wrapper PyObject] -> [2:Memory allocated for SB object (which is just a pointer)] -> [3:The actual LLDB-internal object]
def into_swig_wrapper(cobject, ty, owned=True):
    swig_object = ty.swig_type() # Create an empty wrapper, which will be in an "invalid" state ([2] is null, [3] does not exist).
    addr = int(swig_object.this)
    memmove(addr, byref(cobject), sizeof(ty)) # Replace [2] with a valid pointer.
    swig_object.this.own(owned)
    return swig_object

# The reverse of into_swig_wrapper.
def from_swig_wrapper(swig_object, ty):
    swig_object.this.disown() # We'll be moving this value out, make sure swig_object's destructor does not try to deallocate it.
    addr = int(swig_object.this)
    cobject = ty()
    memmove(byref(cobject), addr, sizeof(ty))
    return cobject

# Generates a FFI type compatible with Rust #[repr(C, i32)] enum
def RustEnum(enum_name, *variants): # type: (str, Tuple[str,type]) -> type
    class V(ctypes.Union):
        _fields_ = variants

    class Enum(ctypes.Structure):
        _fields_ = [('discr', c_int),
                    ('var', V)]
        discr = 0
        def __str__(self):
            name = variants[self.discr][0];
            return '{0}({1})'.format(name, getattr(self.var, name))
        @classmethod
        def __getattr__(cls, name): pass

    for discr, (name, ty) in enumerate(variants):
        def constructor(value, discr=discr, name=name):
            e = Enum(discr)
            e.discr = discr
            setattr(e.var, name, value)
            return e
        setattr(Enum, name, constructor)
    Enum.__name__ = enum_name
    return Enum

def PyResult(name, T): # type: (str, type) -> type
    return RustEnum(name, ('Invalid', c_char), ('Ok', T), ('Err', SBError))

ValueResult = PyResult('ValueResult', SBValue)
BoolResult = PyResult('BoolResult', c_bool)
PyObjectResult = PyResult('PyObjectResult', py_object)

#============================================================================================

display_html = None
save_stdout = None

def initialize(log_level, init_callback_addr, display_html_addr, callback_context):
    global display_html

    logging.getLogger().setLevel(log_level)

    args = [callback_context, postinit, shutdown, compile_code, evaluate, evaluate_as_bool, execute_in_instance, modules_loaded, drop_pyobject]
    init_callback = CFUNCTYPE(None, *([c_void_p] * len(args)))(init_callback_addr)
    init_callback(*args)

    display_html_raw = CFUNCTYPE(None, c_void_p, c_char_p, c_char_p, c_int, c_bool)(display_html_addr)
    display_html = lambda html, title, position, reveal: display_html_raw(
        callback_context, str_to_bytes(html), str_to_bytes(title), position if position != None else -1, reveal)

@CFUNCTYPE(c_bool, c_size_t)
def postinit(console_fd):
    global save_stdout
    # Can't set this from inside SBInterpreter::handle_command() context,
    # because LLDB would restore sys.stdout to the original value.
    if sys.platform.startswith('win32'):
        import msvcrt
        console_fd = msvcrt.open_osfhandle(console_fd, 0)
    save_stdout = sys.stdout
    sys.stdout = os.fdopen(console_fd, 'w', 1) # line-buffered
    return True

@CFUNCTYPE(c_bool)
def shutdown():
    global display_html, save_stdout
    sys.stdout = save_stdout
    display_html = None
    save_stdout = None
    return True

@CFUNCTYPE(c_bool, POINTER(PyObjectResult), POINTER(c_char), c_size_t, POINTER(c_char), c_size_t)
def compile_code(result, expr_ptr, expr_len, filename_ptr, filename_len):
    try:
        expr = ctypes.string_at(expr_ptr, expr_len)
        filename = ctypes.string_at(filename_ptr, filename_len)
        try:
            pycode = compile(expr, filename, 'eval')
        except SyntaxError:
            pycode = compile(expr, filename, 'exec')
        incref(pycode)
        result[0] = PyObjectResult.Ok(pycode)
    except Exception as err:
        log.error(traceback.format_exc())
        error = lldb.SBError()
        error.SetErrorString(str(err))
        error = from_swig_wrapper(error, SBError)
        result[0] = PyObjectResult.Err(error)
    return True

@CFUNCTYPE(c_bool, POINTER(ValueResult), py_object, c_bool, SBExecutionContext)
def evaluate(result, pycode, is_simple_expr, context):
    try:
        context = into_swig_wrapper(context, SBExecutionContext)
        value = evaluate_in_context(pycode, is_simple_expr, context)
        value = to_sbvalue(value, context.target)
        result[0] = ValueResult.Ok(from_swig_wrapper(value, SBValue))
    except Exception as err:
        log.error(traceback.format_exc())
        error = lldb.SBError()
        error.SetErrorString(str(err))
        error = from_swig_wrapper(error, SBError)
        result[0] = ValueResult.Err(error)
    return True

@CFUNCTYPE(c_bool, POINTER(BoolResult), py_object, c_bool, SBExecutionContext)
def evaluate_as_bool(result, pycode, is_simple_expr, context):
    try:
        context = into_swig_wrapper(context, SBExecutionContext)
        value = bool(evaluate_in_context(pycode, is_simple_expr, context))
        result[0] = BoolResult.Ok(value)
    except Exception as err:
        log.error(traceback.format_exc())
        error = lldb.SBError()
        error.SetErrorString(str(err))
        error = from_swig_wrapper(error, SBError)
        result[0] = BoolResult.Err(error)
    return True

@CFUNCTYPE(c_bool, POINTER(BoolResult), py_object, SBDebugger)
def execute_in_instance(result, pycode, debugger):
    try:
        debugger = into_swig_wrapper(debugger, SBDebugger)
        eval_globals = getattr(__main__, debugger.GetInstanceName() + '_dict')
        exec(pycode, eval_globals)
        result[0] = BoolResult.Ok(True)
    except Exception as err:
        log.error(traceback.format_exc())
        error = lldb.SBError()
        error.SetErrorString(str(err))
        error = from_swig_wrapper(error, SBError)
        result[0] = BoolResult.Err(error)
    return True

@CFUNCTYPE(c_bool, POINTER(SBModule), c_size_t)
def modules_loaded(modules_ptr, modules_len):
    return True

@CFUNCTYPE(c_bool, py_object)
def drop_pyobject(obj):
    decref(obj)
    return True


incref = ctypes.pythonapi.Py_IncRef
incref.argtypes = [ctypes.py_object]

decref = ctypes.pythonapi.Py_DecRef
decref.argtypes = [ctypes.py_object]

dummy_sberror = lldb.SBError()

# Convert a native Python object into a SBValue.
def to_sbvalue(value, target):
    value = Value.unwrap(value)

    if isinstance(value, lldb.SBValue):
        return value
    elif value is None:
        ty = target.GetBasicType(lldb.eBasicTypeVoid)
        return target.CreateValueFromData('result', lldb.SBData(), ty)
    elif isinstance(value, bool):
        value = c_int(value)
        asbytes = memoryview(value).tobytes() # type: ignore
        data = lldb.SBData()
        data.SetData(dummy_sberror, asbytes, target.GetByteOrder(), target.GetAddressByteSize()) # borrows from asbytes
        ty = target.GetBasicType(lldb.eBasicTypeBool)
        return target.CreateValueFromData('result', data, ty)
    elif isinstance(value, int):
        value = c_int64(value)
        asbytes = memoryview(value).tobytes() # type: ignore
        data = lldb.SBData()
        data.SetData(dummy_sberror, asbytes, target.GetByteOrder(), target.GetAddressByteSize()) # borrows from asbytes
        ty = target.GetBasicType(lldb.eBasicTypeLongLong)
        return target.CreateValueFromData('result', data, ty)
    elif isinstance(value, float):
        value = c_double(value)
        asbytes = memoryview(value).tobytes() # type: ignore
        data = lldb.SBData()
        data.SetData(dummy_sberror, asbytes, target.GetByteOrder(), target.GetAddressByteSize()) # borrows from asbytes
        ty = target.GetBasicType(lldb.eBasicTypeDouble)
        return target.CreateValueFromData('result', data, ty)
    else: # Fall back to string representation
        value = str(value)
        data = lldb.SBData.CreateDataFromCString(target.GetByteOrder(), target.GetAddressByteSize(), value)
        sbtype_arr = target.GetBasicType(lldb.eBasicTypeChar).GetArrayType(data.GetByteSize())
        return target.CreateValueFromData('result', data, sbtype_arr)

def str_to_bytes(s):
    return s.encode('utf8') if s != None else None

def bytes_to_str(b):
    return b.decode('utf8') if b != None else None

#============================================================================================

def nat_eval(sbframe, expr):
    val = sbframe.FindVariable(expr)
    if not val.IsValid():
        for val_type in [lldb.eValueTypeVariableGlobal,
                         lldb.eValueTypeVariableStatic,
                         lldb.eValueTypeRegister,
                         lldb.eValueTypeConstResult]:
            val = sbframe.FindValue(expr, val_type)
            if val.IsValid():
                break
    if not val.IsValid():
        val = sbframe.GetValueForVariablePath(expr)
    if not val.IsValid():
        val = sbframe.EvaluateExpression(expr)
        err = val.GetError()
        if err.Fail():
            raise Exception(err.GetCString())
    return Value(val)

def evaluate_in_context(code, simple_expr, execution_context):
    frame = execution_context.GetFrame()
    if simple_expr:
        eval_globals = {}
        eval_locals = {}
        eval_globals['__eval'] = lambda expr: nat_eval(frame, expr)
    else:
        debugger = execution_context.GetTarget().GetDebugger()
        eval_globals = getattr(__main__, debugger.GetInstanceName() + '_dict')
        eval_globals['__eval'] = lambda expr: nat_eval(frame, expr)
        eval_locals = {}
        lldb.frame = frame
        lldb.thread = frame.GetThread()
        lldb.process = lldb.thread.GetProcess()
        lldb.target = lldb.process.GetTarget()
        lldb.debugger = lldb.target.GetDebugger()
    return eval(code, eval_globals, eval_locals)
