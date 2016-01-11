import asyncore
import asynchat
import socket
import string
import json

class DebugServer(asyncore.dispatcher):

    def __init__(self, host, port):
        asyncore.dispatcher.__init__(self)
        self.create_socket(socket.AF_INET, socket.SOCK_STREAM)
        self.set_reuse_addr()
        self.bind((host, port))
        self.listen(5)

    def handle_accept(self):
        pair = self.accept()
        if pair is not None:
            sock, addr = pair
            print 'Incoming connection from %s' % repr(addr)
            handler = DebugSessionHandler(sock)
            
class DebugSessionHandler(asynchat.async_chat):

    def __init__(self, sock):
        asynchat.async_chat.__init__(self, sock=sock)
        self.ibuffer = []
        self.obuffer = ""
        self.set_terminator("\r\n\r\n")
        self.reading_headers = True

    def collect_incoming_data(self, data):
        """Buffer the data"""
        self.ibuffer.append(data)
        print "->",data

    def found_terminator(self):
        if self.reading_headers:
            for line in string.split("".join(self.ibuffer), "\r\n"):
                if line.startswith('Content-Length:'):
                    clen = int(string.strip(line[15:]))
                    self.set_terminator(clen)
            self.reading_headers = False
            self.ibuffer = []                    
        else:
            request = json.loads("".join(self.ibuffer))
            print request
            self.reading_headers = True
            self.ibuffer = []        
            
server = DebugServer('localhost', 4711)
asyncore.loop()
