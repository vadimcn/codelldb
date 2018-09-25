#!/usr/bin/python
from __future__ import print_function
import sys, os, subprocess as sp

while True:
    print('----------------------')
    try:
        script = 'script import adapter; adapter.run_tcp_server()'
        sp.call(['lldb', '-b', '-O', script])
    except KeyboardInterrupt:
        break
