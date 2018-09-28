#!/usr/bin/python
import subprocess as sp
import os
import sys

try:
    os.makedirs('debuggee/out')
except Exception:
    pass

if sys.platform == 'win32':
    dll = '.dll'
    dll_flags = []
    exe_flags = []
else:
    dll = '.so'
    dll_flags = ['-fPIC']
    exe_flags = ['-ldl']

try:
    # Make a shared library
    sp.check_call(['c++', 'debuggee/cpp/libdebuggee/libmain.cpp', '-std=c++11', '-shared',
                   '-g', '-o', 'debuggee/out/libdebuggee' + dll] + dll_flags)

    # Compile without debug info
    sp.check_call(['c++', '-c', 'debuggee/cpp/no_line_info.cpp', '-std=c++11', '-o', 'debuggee/out/no_line_info.o'])


    # Intentionally use unnormalized path to source, so we can test breakpoint location filtering.
    sp.check_call(['c++', 'debuggee/cpp/../cpp/./debuggee.cpp', 'debuggee/out/no_line_info.o',
                   '-pthread', '-std=c++11', '-g', '-o', 'debuggee/out/debuggee'] + exe_flags)

except sp.CalledProcessError as e:
    print(e.output)
