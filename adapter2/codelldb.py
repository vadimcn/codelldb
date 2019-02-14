import lldb
import logging
import debugger
from value import Value

logging.basicConfig(level=logging.DEBUG, #filename='/tmp/codelldb.log',
                    format='%(levelname)s(Python) %(asctime)s %(name)s: %(message)s', datefmt='%H:%M:%S')

log = logging.getLogger('codelldb')


# try:
#     import ptvsd
#     ptvsd.enable_attach(address=('0.0.0.0', 3000))
#     #ptvsd.wait_for_attach()
# except:
#     log.warn('Could not import ptvsd')

#============================================================================================

def find_var_in_frame(sbframe, name):
    val = sbframe.FindVariable(name)
    if not val.IsValid():
        for val_type in [lldb.eValueTypeVariableGlobal,
                        lldb.eValueTypeVariableStatic,
                        lldb.eValueTypeRegister,
                        lldb.eValueTypeConstResult]:
            val = sbframe.FindValue(name, val_type)
            if val.IsValid():
                break
    if not val.IsValid():
        val = sbframe.GetValueForVariablePath(name)
    return val

# A dictionary-like object that fetches values from SBFrame (and caches them).
class PyEvalContext(dict):
    def __init__(self, sbframe):
        self.sbframe = sbframe

    def __missing__(self, name):
        val = find_var_in_frame(self.sbframe, name)
        if val.IsValid():
            val = Value(val)
            self.__setitem__(name, val)
            return val
        else:
            raise KeyError(name)

def evaluate_in_frame(script, simple_expr, execution_context):
    frame = execution_context.GetFrame()
    debugger = execution_context.GetTarget().GetDebugger()
    if simple_expr:
        eval_globals = {}
        eval_locals = PyEvalContext(frame)
        eval_globals['__frame_vars'] = eval_locals
    else:
        import __main__
        eval_globals = getattr(__main__, debugger.GetInstanceName() + '_dict')
        eval_globals['__frame_vars'] = PyEvalContext(frame)
        eval_locals = {}
        lldb.frame = frame
        lldb.thread = frame.GetThread()
        lldb.process = lldb.thread.GetProcess()
        lldb.target = lldb.process.GetTarget()
        lldb.debugger = lldb.target.GetDebugger()
    result = eval(script, eval_globals, eval_locals)
    return Value.unwrap(result)

#============================================================================================

type_callbacks = { None:[] }
type_class_mask_union = 0

# callback: Callable[SBModule]
def register_type_callback(callback, language=None, type_class_mask=lldb.eTypeClassAny):
    global type_callbacks, type_class_mask_union
    type_callbacks.setdefault(language, []).append((type_class_mask, callback))
    type_class_mask_union |= type_class_mask

def analyze_module(sbmodule):
    global type_callbacks, type_class_mask_union
    log.info('### analyzing module %s', sbmodule)
    for cu in sbmodule.compile_units:
        callbacks = type_callbacks.get(None) + type_callbacks.get(cu.GetLanguage(), [])
        types = cu.GetTypes(type_class_mask_union)
        for sbtype in types:
            type_class = sbtype.GetTypeClass()
            for type_class_mask, callback in callbacks:
                if type_class & type_class_mask != 0:
                    try:
                        callback(sbtype)
                    except Exception as err:
                        log.error('Type callback %s raised %s', callback, err)

def modules_loaded(modules):
    for module in modules:
        analyze_module(module)
