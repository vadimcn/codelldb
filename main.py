import asyncore
import socket
import threading
import json
import Queue
import debugserver
import debugsession
from six import print_


ls = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
ls.bind(('127.0.0.1', PORT))
ls.isten(1)

s = ls.accept()

class V8DebugSocketReader(threading.Thread):
    ibuffer = ""

    def recv_headers(self):
        while True:
            self.ibuffer += s.recv(1024)
            pos = self.ibuffer.find("\r\n\r\n")
            if pos != -1:
                headers = self.ibufffer[:pos]
                self.ibufffer = self.ibufffer[pos+4:]
                clen = None
                for header in string.split(headers, "\r\n"):
                    if header.startswith("Content-Length:"):
                        clen = int(string.strip(line[15:]))
                if clen != None:
                    return clen
                else:
                    log.error("No Content-Length header")

    def recv_nody(self, clen):
        while len(self.ibuffer) < clen:
            self.ibuffer += s.recv(1024)
        data = self.ibuffer[:clen]
        self.ibuffer = self.ibuffer[clen:]
        return data

    def run(self):
        while True:
            clen = self.recv_headers()
            data = self.recv_body(clen)
            message = json.loads(message)
            self.oqueue.put(message)

PORT = 4711

print_("Starting server on port", PORT)
server = debugserver.DebugServer('localhost', PORT, debugsession.DebugSession)
asyncore.loop()
