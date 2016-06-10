import sys
import os
import logging
import signal

log = logging.getLogger('main')
signal.signal(signal.SIGINT, signal.SIG_DFL)

def init_logging(is_stdio_session):
    log_file = os.getenv('VSCODE_LLDB_LOG', None)
    log_level = 0
    if is_stdio_session and not log_file:
        log_level = logging.ERROR
    logging.basicConfig(level=log_level, filename=log_file, datefmt='%H:%M:%S',
                        format='[%(asctime)s %(name)s] %(message)s')

def run_session(read, write):
    from . import debugsession
    from . import eventloop
    from . import protocolhandler

    event_loop = eventloop.EventLoop()

    proto_handler = protocolhandler.ProtocolHandler(read, write)
    debug_session = debugsession.DebugSession(event_loop, proto_handler.send_message)
    proto_handler.handle_message = event_loop.make_dispatcher(debug_session.handle_message)
    proto_handler.start()
    event_loop.run()

# Run in socket server mode
def run_tcp_server(port=4711):
    import socket
    init_logging(False)
    log.info("Server mode on port %d (Ctrl-C to stop)", port)
    ls = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    ls.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    ls.bind(('127.0.0.1', port))
    ls.listen(1)

    while True:
        conn, addr = ls.accept()
        conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        log.info("New connection from %s", addr)
        run_session(conn.recv, conn.send)
        conn.close()
        log.info("Debug session ended. Waiting for new connections.")

# Single-session run using the specified input and output fds
def run_stdio_session(ifd=0, ofd=1):
    init_logging(True)
    log.info("Single-session mode on fds (%d,%d)", ifd, ofd)
    r = lambda n: os.read(ifd, n)
    w = lambda data: os.write(ofd, data)
    run_session(r, w)
    log.info("Debug session ended. Exiting.")
