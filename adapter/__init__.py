import os
import logging
import signal
import lldb

log = logging.getLogger(__name__)

signal.signal(signal.SIGINT, signal.SIG_DFL)

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

def configLogging(level):
    logging.basicConfig(level=level, stream=os.fdopen(2, "w"))

# Run in socket server mode
def server(port=4711, loglevel=0):
    import socket

    configLogging(loglevel)
    log.info("Server mode on port %d (Ctrl-C to stop)", port)
    log.info("%s", lldb.debugger.GetVersionString())
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
def stdio(ifd, ofd, loglevel=40):
    configLogging(loglevel)
    log.info("Single-session mode on fds (%d,%d)", ifd, ofd)
    log.info("%s", lldb.debugger.GetVersionString())
    r = lambda n: os.read(ifd, n)
    w = lambda data: os.write(ofd, data)
    run_session(r, w)
    log.info("Debug session ended. Exiting.")
