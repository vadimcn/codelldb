import sys
import os
import logging
import subprocess
import traceback

def setup_lldb():
    # Ask LLDB where its Python modules live
    lldb_pypath = subprocess.check_output(['lldb', '--python-path']).strip()
    log.info('LLDB python path: %s', lldb_pypath)
    sys.path[:0] = [lldb_pypath]

def run_session(read, write):
    import debugsession
    import eventloop
    import protocolhandler

    event_loop = eventloop.EventLoop()

    proto_handler = protocolhandler.ProtocolHandler(read, write)
    debug_session = debugsession.DebugSession(event_loop, proto_handler.send_message)

    proto_handler.start(debug_session.handle_request)
    event_loop.run()
    proto_handler.shutdown()

# Run in socket server mode
def run_tcp_server(port=4711):
    import socket
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
    log.info("Single-session mode on fds (%d,%d)", ifd, ofd)
    r = lambda n: os.read(ifd, n)
    w = lambda data: os.write(ofd, data)
    run_session(r, w)
    log.info("Debug session ended. Exiting.")

# entry
stdio_session = ('--stdio' in sys.argv)
log_file = os.getenv('VSCODE_LLDB_LOG', None)
log_level = 0
if stdio_session and not log_file:
    log_level = logging.ERROR
logging.basicConfig(level=log_level, filename=log_file)
log = logging.getLogger('main')

try:
    setup_lldb()
    if stdio_session:
        run_stdio_session()
    else:
        run_tcp_server()
except KeyboardInterrupt:
    pass
except Exception as e:
    tb = traceback.format_exc(e)
    log.error(tb)
