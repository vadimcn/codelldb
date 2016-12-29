#!/usr/bin/python
import subprocess as sp

try:
    sp.check_call(['c++', 'src/tests/debuggee.cpp', '-pthread', '-std=c++11', '-g', '-o', 'out/tests/debuggee'])
except sp.CalledProcessError as e:
    print(e.output)

print('Done')
