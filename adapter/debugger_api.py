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

def display_html(uri, title=None, position=None, content={}):
    request_body = { 'uri': uri, 'position': position, 'title': title, 'content': content }
    debugsession.DebugSession.current.display_html(request_body)

def register_content_provider(provider):
    debugsession.DebugSession.current.provide_content = provider

__all__ = ['evaluate', 'stop_if', 'preview_html', 'register_content_provider']
