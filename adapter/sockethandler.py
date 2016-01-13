import threading
import json
import string
import logging

log = logging.getLogger(__name__)

class V8SocketHandler:

    def __init__(self, socket, handle_request):
        self.socket = socket
        self.handle_request = handle_request
        self.ibuffer = ""
        self.stopping = False
        self.reader_thread = threading.Thread(None, self.pump_requests)
        self.reader_thread.start()

    def __del__(self):
        self.stopping = True
        self.socket.close()
        self.reader_thread.join()

    def recv_headers(self):
        while True:
            pos = self.ibuffer.find("\r\n\r\n")
            if pos != -1:
                headers = self.ibuffer[:pos]
                self.ibuffer = self.ibuffer[pos+4:]
                clen = None
                for header in string.split(headers, "\r\n"):
                    if header.startswith("Content-Length:"):
                        clen = int(string.strip(header[15:]))
                if clen != None:
                    return clen
                else:
                    log.error("No Content-Length header")

            data = self.socket.recv(1024)
            self.ibuffer += data

    def recv_body(self, clen):
        while len(self.ibuffer) < clen:
            data = self.socket.recv(1024)
            self.ibuffer += data
        data = self.ibuffer[:clen]
        self.ibuffer = self.ibuffer[clen:]
        return data

    def pump_requests(self):
        while not self.stopping:
            clen = self.recv_headers()
            data = self.recv_body(clen)
            message = json.loads(data)
            log.info("-> %s", data)
            self.handle_request(message)

    def send_message(self, message):
        data = json.dumps(message)
        log.info("<- %s", data)
        self.socket.send("Content-Length: %d\r\n\r\n" % len(data))
        self.socket.send(data)
