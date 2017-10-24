#!/usr/bin/python
import subprocess as sp

try:
    # Intentionally use unnormalized path to source, so we can test breakpoint location filtering.
    sp.check_call(['c++', '-c', 'extension/tests/cpp/no_line_info.cpp', '-std=c++11'])
    sp.check_call(['c++', 'extension/tests/cpp/../cpp/./debuggee.cpp', 'no_line_info.o', '-pthread', '-std=c++11', '-g', '-o', 'out/tests/debuggee'])
except sp.CalledProcessError as e:
    print(e.output)
