import sys
PY2 = sys.version_info[0] == 2
if PY2:
    is_string = lambda v: isinstance(v, basestring)
    xrange = xrange
else:
    is_string = lambda v: isinstance(v, str) 
    xrange = range
import adapter.main
