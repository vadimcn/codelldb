import sys
import lldb
import codelldb
from value import Value

def evaluate(expr, unwrap=False):
    exec_context = lldb.SBExecutionContext(lldb.frame)
    value = codelldb.evaluate_in_context(expr, True, exec_context)
    return Value.unwrap(value) if unwrap else value

def wrap(obj):
    return obj if type(obj) is Value else Value(obj)

def unwrap(obj):
    return Value.unwrap(obj)

def display_html(html, title=None, position=None, reveal=False):
    codelldb.display_html(html, title, position, reveal)

def register_type_callback(callback, language=None, type_class_mask=lldb.eTypeClassAny):
    raise NotImplementedError('This API has been removed')

def register_content_provider(provider):
    raise NotImplementedError('This API has been removed')

def stop_if(cond, handler):
    import warnings
    warnings.warn('deprecated', DeprecationWarning)

    if cond:
        handler()
        return True
    else:
        return False

__all__ = ['evaluate', 'wrap', 'unwrap', 'display_html', 'register_type_callback', 'register_content_provider', 'stop_if']
