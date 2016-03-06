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
        self.ibuffer = b''
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
            pos = self.ibuffer.find(b'\r\n\r\n')
            if pos != -1:
                headers = self.ibuffer[:pos]
                self.ibuffer = self.ibuffer[pos+4:]
                clen = None
                for header in headers.split(b'\r\n'):
                    if header.startswith(b'Content-Length:'):
                        clen = int(header[15:].strip())
                if clen != None:
                    return clen
                else:
                    log.error('No Content-Length header')

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
                data = data.decode('utf-8')
                log.debug('-> %s', data)
                message = json.loads(data)
                self.handle_request(message)
        except StopIteration: # Thrown when read() returns 0
            pass
        except Exception as e:
            log.error(e)

    def send_message(self, message):
        data = json.dumps(message)
        log.debug('<- %s', data)
        data = data.encode('utf-8')
        self.write(b'Content-Length: %d\r\n\r\n' % len(data))
        self.write(data)
