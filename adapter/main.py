import sys
import os
import logging
import signal
import socket
import traceback
import errno

log = logging.getLogger('main')
signal.signal(signal.SIGINT, signal.SIG_DFL)

if 'linux' in sys.platform or 'darwin' in sys.platform:
    # Limit memory usage to 16GB to prevent runaway visualizers from killing the machine
    import resource
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    resource.setrlimit(resource.RLIMIT_AS, (16 * 1024**3, hard))

def init_logging(log_file, log_level):
    logging.basicConfig(level=log_level, filename=log_file, datefmt='%H:%M:%S',
                        format='[%(asctime)s %(name)s] %(message)s')

def run_session(read, write, ext_channel_port):
    try:
        from .wireprotocol import DebugServer
        from .debugsession import DebugSession
        from .eventloop import EventLoop

        event_loop = EventLoop()

        # Attach debug protocol listener to the main communication channel.
        debug_server = DebugServer()
        debug_server.set_channel(read, write)

        # Establish auxilary channel to VSCode extension.
        if ext_channel_port is not None:
            ext_conn = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            ext_conn.connect(('127.0.0.1', ext_channel_port))
            ext_conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            ext_conn.settimeout(0.5)

            ext_server = DebugServer()
            ext_server.set_channel(ext_conn.recv, ext_conn.sendall)
            send_ext_message = ext_server.send_message
        else:
            ext_conn = None
            ext_server = None
            send_ext_message = None

        # Create DebugSession, tell it how to send messages back to the clients.
        debug_session = DebugSession(event_loop, debug_server.send_message, send_ext_message)

        # Start up worker threads.
        debug_server.handle_message = event_loop.make_dispatcher(debug_session.handle_message)
        token1 = debug_server.start()
        if ext_server is not None:
            ext_server.handle_message = event_loop.make_dispatcher(debug_session.handle_extension_message)
            token2 = ext_server.start()

        # Run event loop until DebugSession breaks it.
        event_loop.run()
    except Exception as e:
        log.error('%s', traceback.format_exc())
    finally:
        if ext_conn is not None:
            ext_conn.close()

# Run in socket server mode
def run_tcp_server(port=4711, multiple=True, ext_channel_port=None):
    init_logging(None, logging.NOTSET)
    log.info('Server mode on port %d (Ctrl-C to stop)', port)
    ls = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    ls.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    ls.bind(('127.0.0.1', port))
    ls.listen(1)

    while True:
        conn, addr = ls.accept()
        conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        log.info('New connection from %s', addr)
        run_session(conn.recv, conn.sendall, ext_channel_port)
        conn.close()
        if multiple:
            log.info('Debug session ended. Waiting for new connections.')
        else:
            log.info('Debug session ended.')
            break
    ls.close()

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

# Single-session run using the specified input and output fds
def run_stdio_session(ifd=0, ofd=1, ext_channel_port=None, log_file=None, log_level=logging.CRITICAL):
    if log_file is not None:
        import base64
        log_file = base64.b64decode(log_file)
    init_logging(log_file, log_level)
    log.info('Single-session mode on fds (%d,%d)', ifd, ofd)
    run_session(lambda n: os_read(ifd, n), lambda data: os_write_all(ofd, data), ext_channel_port)
    log.info('Debug session ended. Exiting.')
