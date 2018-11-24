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

__all__ = ['evaluate', 'stop_if', 'preview_html']
