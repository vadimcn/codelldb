from __future__ import print_function
import sys
import os
import logging
import signal
import socket
import traceback
import errno
from .wireprotocol import DebugServer
from .debugsession import DebugSession
from .eventloop import EventLoop
from . import PY2, is_string, debugger_api

log = logging.getLogger('main')
signal.signal(signal.SIGINT, signal.SIG_DFL)

sys.modules['debugger'] = debugger_api

if 'linux' in sys.platform or 'darwin' in sys.platform:
    # Limit memory usage to 16GB to prevent runaway visualizers from killing the machine
    import resource
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    resource.setrlimit(resource.RLIMIT_AS, (16 * 1024**3, hard))

def decode_params(params):
    try:
        if isinstance(params, dict):
            return params
        elif is_string(params):
            import base64, json
            return json.loads(base64.b64decode(params).decode('utf8'))
    except e as Exception:
        print(e)
    return {}

def init_logging(params):
    log_file = params.get('logFile', None)
    log_level = params.get('logLevel', logging.CRITICAL)
    logging.basicConfig(level=log_level, filename=log_file, datefmt='%H:%M:%S',
                        format='[%(asctime)s %(name)s] %(message)s')

def run_session(read, write, params):
    event_loop = EventLoop()

    # Attach debug protocol listener to the main communication channel.
    debug_server = DebugServer()
    debug_server.set_channel(read, write)

    # Create DebugSession, tell it how to send messages back to the clients.
    debug_session = DebugSession(params, event_loop, debug_server.send_message)

    # Start up worker threads.
    debug_server.handle_message = event_loop.make_dispatcher(debug_session.handle_message)
    token = debug_server.start()

    # Run event loop until DebugSession breaks it.
    event_loop.run()

from os import read as os_read, write as os_write
def os_write_all(ofd, data):
    while True:
        try:
            n = os_write(ofd, data)
        except OSError as e: # This may happen if we fill-up the output pipe's buffer.
            if e.errno != errno.EAGAIN:
                raise
            n = 0
        if n == len(data):
            return
        data = data[n:]

# Single session on top of the specified fds
def run_pipe_session(ifd, ofd, params=None):
    try:
        params = decode_params(params)
        init_logging(params)
        log.info('Single-session mode on fds (%d, %d)', ifd, ofd)
        run_session(lambda n: os_read(ifd, n), lambda data: os_write_all(ofd, data), params)
        log.info('Debug session has ended. Exiting.')
    except Exception as e:
        log.critical('%s', traceback.format_exc())

# Single session on top of stdin & stdout
def run_stdio_session(params=None):
    try:
        params = decode_params(params)
        init_logging(params)
        # Keeping all imported components from spamming stdout is pretty futile;
        # just relocate stdio to different fds.
        ifd = os.dup(0)
        ofd = os.dup(1)
        os.dup2(os.open(os.devnull, os.O_RDONLY), 0)
        os.dup2(os.open(os.devnull, os.O_WRONLY), 1)
        log.info('Single-session mode on stdio')
        run_session(lambda n: os_read(ifd, n), lambda data: os_write_all(ofd, data), params)
        log.info('Debug session has ended. Exiting.')
    except Exception as e:
        log.critical('%s', traceback.format_exc())

# Single session on the specified TCP port.
def run_tcp_session(port, params=None):
    try:
        params = decode_params(params)
        init_logging(params)
        ls = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        ls.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        ls.bind(('127.0.0.1', port))
        ls.listen(1)
        ls_port = ls.getsockname()[1]
        print('Listening on port', ls_port) # Let the parent process know which port we are listening on.
        sys.stdout.flush()
        conn, addr = ls.accept()
        conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        log.info('New connection from %s', addr)
        run_session(conn.recv, conn.sendall, params)
        log.info('Debug session has ended. Exiting.')
        conn.close()
        ls.close()
    except Exception as e:
        log.critical('%s', traceback.format_exc())

# Run in socket server mode
def run_tcp_server(port=4711, params={'logLevel':0}):
    print('Server mode on port %d (Ctrl-C to stop)' % port)
    while True:
        run_tcp_session(port, params)
        print('Debug session has ended. Waiting for new connections.')
