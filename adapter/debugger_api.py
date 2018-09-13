import sys
import lldb
from . import debugsession
from . import expressions

def evaluate(expr):
    return debugsession.DebugSession.current.evaluate_expr_in_frame(expr, lldb.frame)

def unwrap(obj):
    return expressions.Value.unwrap(obj)

def wrap(obj):
    return obj if type(obj) is expressions.Value else expressions.Value(obj)

def stop_if(cond, handler):
    if cond:
        handler()
        return True
    else:
        return False

def display_html(html, title=None, position=None, reveal=False):
    request_body = { 'html': html, 'position': position, 'title': title, 'reveal': reveal }
    debugsession.DebugSession.current.display_html(request_body)

def register_content_provider(provider):
    debugsession.DebugSession.current.provide_content = provider

def register_type_callback(callback, language=None, type_class_mask=lldb.eTypeClassAny):
    expressions.register_type_callback(callback, language, type_class_mask)

__all__ = ['evaluate', 'unwrap', 'wrap', 'stop_if', 'display_html', 'register_type_callback']
