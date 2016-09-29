import json
import logging
import socket
from .workerthread import WorkerThread

log = logging.getLogger('wireprotocol')

# Wire protocol handler for the main debug connection
class DebugServer(WorkerThread):
    handle_message = None

    # `read(N)`: callback to read up to N bytes from the input stream.
    # `write(buffer)`: callback to write bytes into the output stream.
    def reset(self, read, write):
        self.read = read
        self.write = write
        self.ibuffer = b''

    def thread_proc(self):
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
        data = data.encode('utf-8')
        self.with_timeout(self.write, b'Content-Length: %d\r\n\r\n' % len(data))
        self.with_timeout(self.write, data)


# Wire protocol handler for the auxilary connection to VSCode extension
class ExtensionServer(DebugServer):
    def __init__(self):
        DebugServer.__init__(self)
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.bind(('127.0.0.1', 0))
        self.sock.listen(1)
        addr, port = self.sock.getsockname()
        self.port = port

    def thread_proc(self):
        try:
            self.sock.settimeout(0.3)
            conn, addr = self.with_timeout(self.sock.accept)
            conn.settimeout(0.3)
            self.sock.close()
            DebugServer.reset(self, conn.recv, conn.send)
            DebugServer.thread_proc(self)
        except StopIteration:
            pass
