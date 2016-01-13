import os
import socket
import subprocess
import string

class Terminal:
    def __init__(self, tty, socket):
        self.tty = tty
        self.socket = socket

    def __del__(self):
        self.socket.close()

TIMEOUT = 1 # Timeout in seconds for child opening a socket and sending the tty name

def create():
    socket_path = '/tmp/mi-debug-%d.sock' % os.getpid()
    try: os.unlink(socket_path)
    except OSError: pass
    ls = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    ls.bind(socket_path)
    ls.listen(1)
    subprocess.Popen(['x-terminal-emulator', '-e', 'bash -c "tty | nc -U %s -q -1"' % socket_path]);

    try:
        ls.settimeout(TIMEOUT)
        conn, addr = ls.accept()
        os.unlink(socket_path)

        conn.settimeout(TIMEOUT)
        data = ""
        while True:
            data += conn.recv(32)
            lines = string.split(data, "\n")
            if len(lines) > 1:
                return Terminal(lines[0], conn)

    except (OSError, socket.timeout):
        raise Exception("Failed to create a new terminal")