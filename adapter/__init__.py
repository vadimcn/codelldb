import logging
logging.basicConfig(level=10)

import socket
import sockethandler
import debugsession
import eventloop
import signal

log = logging.getLogger()

signal.signal(signal.SIGINT, signal.SIG_DFL)

PORT = 4711
logging.info("Listening on port %d", PORT)
ls = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
ls.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
ls.bind(('127.0.0.1', PORT))
ls.listen(1)

conn, addr = ls.accept()
conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)

logging.info("New connection from %s", addr)

event_loop = eventloop.EventLoop()

def handle_request(msg):
    global debug_session
    event_loop.dispatch1(debug_session.on_request, msg)

def send_message(msg):
    global socket_handler
    socket_handler.send_message(msg)

debug_session = debugsession.DebugSession(event_loop, send_message)
socket_handler = sockethandler.V8SocketHandler(conn, handle_request)

event_loop.run()
