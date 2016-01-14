import threading
import json
import string
import logging
import sys

log = logging.getLogger(__name__)

class ProtocolHandler:

    def __init__(self, read, write):
        self.read = read
        self.write = write
        self.ibuffer = ""
        self.stopping = False

    def start(self, handle_request):
        self.handle_request = handle_request
        self.reader_thread = threading.Thread(None, self.pump_requests)
        self.reader_thread.start()

    def shutdown(self):
        self.stopping = True
        self.reader_thread.join()
        self.handle_request = None

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

            data = self.read(1024)
            if len(data) == 0:
                raise StopIteration()
            self.ibuffer += data

    def recv_body(self, clen):
        while len(self.ibuffer) < clen:
            data = self.read(1024)
            self.ibuffer += data
        data = self.ibuffer[:clen]
        if len(data) == 0:
            raise StopIteration()
        self.ibuffer = self.ibuffer[clen:]
        return data

    def pump_requests(self):
        try:
            while not self.stopping:
                clen = self.recv_headers()
                data = self.recv_body(clen)
                message = json.loads(data)
                log.debug("-> %s", data)
                self.handle_request(message)
        except StopIteration: # Thrown when read() returns 0
            pass

    def send_message(self, message):
        data = json.dumps(message)
        log.debug("<- %s", data)
        self.write("Content-Length: %d\r\n\r\n" % len(data))
        self.write(data)
