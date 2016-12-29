#!/usr/bin/python
import sys, subprocess as sp

if 'win32' in sys.platform:
    extinfo = r'c:\\temp\\vscode-lldb-session'
else:
    extinfo = '/tmp/vscode-lldb-session'

while True:
    print('----------------------')
    try:
        sp.call(['lldb', '-b', '-O' 'script import adapter; adapter.run_tcp_server(multiple=False, extinfo="%s")' % extinfo])
    except KeyboardInterrupt:
        break
