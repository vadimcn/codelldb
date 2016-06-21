import json
import logging
from .workerthread import WorkerThread

log = logging.getLogger('protocolhandler')

class ProtocolHandler(WorkerThread):
    # `read(N)`: callback to read up to N bytes from the input stream.
    # `write(buffer)`: callback to write bytes into the output stream.
    def __init__(self, read, write):
        WorkerThread.__init__(self)
        self.read = read
        self.write = write
        self.ibuffer = b''
       	self.stopping = False
        self.handle_message = None

    def run(self):
        assert self.handle_message is not None
        try:
            while not self.stopping:
                clen = self.recv_headers()
                data = self.recv_body(clen)
                data = data.decode('utf-8')
                log.debug('rx: %s', data)
                message = json.loads(data)
                self.handle_message(message)
            log.debug('Shutting down')
        except StopIteration: # Thrown when read() returns 0
            self.handle_message(None)
        except Exception as e:
            log.error(e)
            self.handle_message(None)

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

    def send_message(self, message):
        data = json.dumps(message)
        log.debug('tx: %s', data)
        data = data.encode('utf-8')
        self.write(b'Content-Length: %d\r\n\r\n' % len(data))
        self.write(data)
