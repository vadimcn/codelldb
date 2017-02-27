import sys
PY2 = sys.version_info[0] == 2
if PY2:
    is_string = lambda v: isinstance(v, basestring)
    xrange = xrange
else:
    is_string = lambda v: isinstance(v, str) 
    xrange = range
import adapter.main

def preview_html(uri, title=None, position=None, content={}):
    from adapter.debugsession import DebugSession
    request_body = { 'uri': uri, 'position': position, 'title': title, 'content': content }
    DebugSession.current.preview_html(request_body)

def register_content_provider(provider):
    from adapter.debugsession import DebugSession
    DebugSession.current.provide_content = provider

__all__ = ['preview_html', 'register_content_provider']
