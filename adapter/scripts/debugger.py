import lldb
from codelldb import codelldb
from codelldb.value import Value
from codelldb.debug_info import DebugInfoCommand


def evaluate(expr, unwrap=False):
    value = codelldb.nat_eval(lldb.frame, expr)
    return Value.unwrap(value) if unwrap else value


def wrap(obj):
    return obj if type(obj) is Value else Value(obj)


def unwrap(obj):
    return Value.unwrap(obj)


def display_html(html, title=None, position=None, reveal=False):
    codelldb.display_html(html, title, position, reveal)


__all__ = ['evaluate', 'wrap', 'unwrap', 'display_html']


def __lldb_init_module(debugger, internal_dict):
    debugger.HandleCommand('command script add -c debugger.DebugInfoCommand debug_info')
