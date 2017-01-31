#!/usr/bin/python
import sys, os, subprocess as sp

if 'win32' in sys.platform:
    extinfo = r'c:\\temp\\vscode-lldb-session'
else:
    extinfo = '/tmp/vscode-lldb-session'

while True:
    print('----------------------')
    try:
        script = 'script import adapter; adapter.run_tcp_server(multiple=False, extinfo="%s")' % extinfo
        sp.call(['lldb', '-b', '-O', script])
    except KeyboardInterrupt:
        break
