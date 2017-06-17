#!/usr/bin/python
import sys, os, subprocess as sp

while True:
    print('----------------------')
    try:
        script = 'script import adapter; adapter.main.run_tcp_server()'
        sp.call(['lldb', '-b', '-O', script])
    except KeyboardInterrupt:
        break
