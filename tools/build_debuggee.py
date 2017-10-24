#!/usr/bin/python
import subprocess as sp
import os

try:
    os.makedirs('out/debuggee')
except Exception:
    pass

try:
    sp.check_call(['c++', '-c', 'debuggee/cpp/no_line_info.cpp', '-std=c++11', '-o', 'out/debuggee/no_line_info.o'])
    # Intentionally use unnormalized path to source, so we can test breakpoint location filtering.
    sp.check_call(['c++', 'debuggee/cpp/../cpp/./debuggee.cpp', 'out/debuggee/no_line_info.o', '-pthread', '-std=c++11', '-g', '-o', 'out/debuggee/debuggee'])
except sp.CalledProcessError as e:
    print(e.output)
