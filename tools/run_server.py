#!/usr/bin/python
import sys, os, subprocess as sp

if 'win32' in sys.platform:
    extinfo = r'c:\\temp\\vscode-lldb-session'
else:
    extinfo = '/tmp/vscode-lldb-session'

while True:
    print('----------------------')
    try:
        script = ('script import adapter\r\n' +
                  'script adapter.run_tcp_server(multiple=False, extinfo="%s")\r\n' % extinfo +
                  'exit\r\n')
        read_fd, write_fd = os.pipe()
        os.write(write_fd, script.encode('utf-8'))
        sp.call(['lldb'], stdin=read_fd)
    except KeyboardInterrupt:
        break
