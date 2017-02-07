import os
import socket
import subprocess
import string
import logging

log = logging.getLogger('terminal')

class Terminal:
    def __init__(self, tty, socket):
        self.tty = tty
        self.socket = socket

    def __del__(self):
        self.socket.close()

TIMEOUT = 3 # Timeout in seconds for child opening a socket and sending the tty name

def create(spawn_terminal=None):
    ls = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    ls.bind(('127.0.0.1', 0))
    ls.listen(1)
    addr, port = ls.getsockname()
    # terminal.sh opens a TCP connection, sends output of `tty`, 
    # waits till the socket gets closed from our end
    args = [os.path.join(os.path.dirname(__file__), 'terminal.sh'), str(port)]
    if spawn_terminal is not None:
        spawn_terminal(args)
    else:
        subprocess.Popen(['x-terminal-emulator', '-e', ' '.join(command)]);

    try:
        ls.settimeout(TIMEOUT)
        conn, addr = ls.accept()
        conn.settimeout(TIMEOUT)
        output = ''
        while True:
            data = conn.recv(32)
            if len(data) == 0:
                reason = 'connection aborted'
                break
            log.info('received %s', data)
            output += data
            lines = string.split(output, '\n')
            if len(lines) > 1:
                return Terminal(lines[0], conn)
    except (OSError, socket.timeout):
        reason = 'timeout'

    raise Exception('Failed to create a new terminal: %s' % reason)
