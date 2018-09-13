from __future__ import print_function
import sys
import os
import time
import socket
import argparse
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--launch-adapter')
parser.add_argument('--lldb')
parser.add_argument('--wait-port')

args = parser.parse_args()

if args.launch_adapter:
    lldb = args.lldb or 'lldb'
    cmd = [lldb, '-b',
        '-O', 'command script import %s' % args.launch_adapter,
        '-O', 'script import ptvsd; ptvsd.enable_attach(address=("0.0.0.0", 3000)); ptvsd.wait_for_attach(); adapter.run_tcp_session(4711)',
    ]
    print('Launching', cmd)
    subprocess.Popen(cmd, preexec_fn=lambda: os.setsid())

if args.wait_port:
    port = int(args.wait_port)
    print('Waiting for port %d' % port)

    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    while True:
        result = sock.connect_ex(('127.0.0.1', port))
        if result == 0:
            break
        time.sleep(0.5)

    print('Port opened')
    sock.shutdown(socket.SHUT_WR)
    sock.close()
