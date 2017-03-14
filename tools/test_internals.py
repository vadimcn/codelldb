#!/usr/bin/python
# Execute tests in Python code
import sys
import os

def where(program):
    for path in os.environ['PATH'].split(os.pathsep):
        path = path.strip('"')
        exe_file = os.path.join(path, program)
        if os.path.isfile(exe_file) and os.access(exe_file, os.X_OK):
            return exe_file

if 'darwin' in sys.platform:
    sys.path.append('/Applications/Xcode.app/Contents/SharedFrameworks/LLDB.framework/Resources/Python')
elif 'linux' in sys.platform:
    bindir = os.path.dirname(os.path.realpath(where('lldb')))
    pydir = os.path.join(bindir, '..', 'lib', 'python2.7', 'site-packages')
    sys.path.append(pydir)
elif 'win32' in sys.platform:
    bindir = os.path.dirname(os.path.realpath(where('lldb.exe')))
    pydir = os.path.join(bindir, '..', 'lib', 'site-packages')
    sys.path.append(pydir)
else:
    print('Unknown OS')
sys.path.append('.')

from adapter import expressions
expressions.run_tests()
print('Success')
