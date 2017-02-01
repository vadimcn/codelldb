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

def init_logging(is_stdio_session):
    log_file = os.getenv('VSCODE_LLDB_LOG', None)
    log_level = 0
    if is_stdio_session and not log_file:
        log_level = logging.ERROR
    logging.basicConfig(level=log_level, filename=log_file, datefmt='%H:%M:%S',
                        format='[%(asctime)s %(name)s] %(message)s')

def run_session(read, write, extinfo):
    try:
        from .wireprotocol import DebugServer, ExtensionServer
        from .debugsession import DebugSession
        from .eventloop import EventLoop

        event_loop = EventLoop()

        # Attach debug protocol listener to the main communication channel
        debug_server = DebugServer()
        debug_server.reset(read, write)

        # Create extension server and announce its port number
        ext_server = ExtensionServer()
        if extinfo is not None:
            open(extinfo, 'wb').write(str(ext_server.port).encode('utf-8'))
            log.info('Extension server port info (%d) written to %s', ext_server.port, extinfo) 
        
        # Create DebugSession, tell it how to send messages back to the clients
        debug_session = DebugSession(event_loop, debug_server.send_message, ext_server.send_message)

        # Wire up the received message handlers
        debug_server.handle_message = event_loop.make_dispatcher(debug_session.handle_message)
        ext_server.handle_message = event_loop.make_dispatcher(debug_session.handle_extension_message)

        # Start up worker threads
        token1 = debug_server.start()
        token2 = ext_server.start()

        # Run event loop until DebugSession breaks it
        event_loop.run()
    except Exception as e:
        log.error('%s', traceback.format_exc())
    finally:
        if extinfo is not None:
            os.unlink(extinfo)

# Run in socket server mode
def run_tcp_server(port=4711, multiple=True, extinfo=None):
    init_logging(False)
    log.info('Server mode on port %d (Ctrl-C to stop)', port)
    ls = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    ls.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    ls.bind(('127.0.0.1', port))
    ls.listen(1)

    while True:
        conn, addr = ls.accept()
        conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        log.info('New connection from %s', addr)
        run_session(conn.recv, conn.sendall, extinfo)
        conn.close()
        if multiple:
            log.info('Debug session ended. Waiting for new connections.')
        else:
            log.info('Debug session ended.')
            break
    ls.close()

from os import read as os_read, write as os_write
def os_write_all(ofd, data):
    n = os_write(ofd, data)
    while n < len(data): # This may happen when fill-up the output pipe's buffer.
        data = data[n:]
        try:
            n = os_write(ofd, data)
        except OSError as e:
            if e.errno != errno.EAGAIN:
                raise
            n = 0

# Single-session run using the specified input and output fds
def run_stdio_session(ifd=0, ofd=1, extinfo=None):
    init_logging(True)
    log.info('Single-session mode on fds (%d,%d)', ifd, ofd)
    run_session(lambda n: os_read(ifd, n), lambda data: os_write_all(ofd, data), extinfo)
    log.info('Debug session ended. Exiting.')
