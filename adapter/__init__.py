import sys

PY2 = sys.version_info[0] == 2
if PY2:
    is_string = lambda v: isinstance(v, basestring)
    # python2-based LLDB accepts utf8-encoded ascii strings only.
    to_lldb_str = lambda s: s.encode('utf8', 'backslashreplace') if isinstance(s, unicode) else s
    from_lldb_str = lambda s: s.decode('utf8', 'replace')
    xrange = xrange
else:
    is_string = lambda v: isinstance(v, str)
    to_lldb_str = str
    from_lldb_str = str
    xrange = range

import adapter.main
