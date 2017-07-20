import json
import logging
import socket
from .workerthread import WorkerThread

log = logging.getLogger('wireprotocol')

# Wire protocol handler for the main debug connection
class DebugServer(WorkerThread):
    handle_message = None

    # `read(N)`: callback to read up to N bytes from the input stream.
    # `write_all(buffer)`: callback to write bytes into the output stream.
    def set_channel(self, read, write_all):
        self.read = read
        self.write_all = write_all
        self.ibuffer = b''

    def thread_proc(self):
        assert self.handle_message is not None
        try:
            while not self.stopping:
                clen = self.recv_headers()
                data = self.recv_body(clen)
                data = data.decode('utf8')
                log.debug('rx: %s', data)
                message = json.loads(data)
                self.handle_message(message)
            log.debug('Shutting down')
        except StopIteration: # Thrown when read() returns 0
            log.debug('Disconnected')
            self.handle_message(None)

    # Execute I/O operation, which may have a timeout associated with it.
    def with_timeout(self, operation, *args):
        while not self.stopping:
            try:
                return operation(*args)
            except socket.timeout:
                pass
        raise StopIteration()

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

            data = self.with_timeout(self.read, 1024)
            if len(data) == 0:
                raise StopIteration()
            self.ibuffer += data

    def recv_body(self, clen):
        while len(self.ibuffer) < clen:
            data = self.with_timeout(self.read, 1024)
            if len(data) == 0:
                raise StopIteration()
            self.ibuffer += data
        data = self.ibuffer[:clen]
        self.ibuffer = self.ibuffer[clen:]
        return data

    json_separators = (',', ':')
    def send_message(self, message):
        data = json.dumps(message, separators=self.json_separators)
        log.debug('tx: %s', data)
        data = data.encode('utf8')
        self.with_timeout(self.write_all, b'Content-Length: %d\r\n\r\n' % len(data))
        self.with_timeout(self.write_all, data)
