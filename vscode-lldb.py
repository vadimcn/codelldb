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
            self.dispatch(request)
            self.reading_headers = True
            self.ibuffer = []

    def dispatch(self, request):
        command =  request["command"]
        args = request["arguments"]
        self.dispatch_map[command](self, args)

    def initialize_request(self, args):
        pass
    def launch_request(self, args):
        pass
    def attach_request(self, args):
        pass
    def disconnect_request(self, args):
        pass
    def set_breakpoints_request(self, args):
        pass
    def set_exception_breakpoints_request(self, args):
        pass
    def continue_request(self, args):
        pass
    def next_request(self, args):
        pass
    def step_in_request(self, args):
        pass
    def step_out_request(self, args):
        pass
    def pause_request(self, args):
        pass
    def stack_trace_request(self, args):
        pass
    def scopes_request(self, args):
        pass
    def variables_request(self, args):
        pass
    def source_request(self, args):
        pass
    def threads_request(self, args):
        pass
    def evaluate_request(self, args):
        pass

    dispatch_map = {
        "initialize": initialize_request,
        "launch": launch_request,
        "attach": attach_request,
        "disconnect": disconnect_request,
        "setBreakpoints": set_breakpoints_request,
        "setExceptionBreakpoints": set_exception_breakpoints_request,
        "continue": continue_request,
        "next": next_request,
        "stepIn": step_in_request,
        "stepOut": step_out_request,
        "pause": pause_request,
        "stackTrace": stack_trace_request,
        "scopes": scopes_request,
        "variables": variables_request,
        "source": source_request,
        "threads": threads_request,
        "evaluate": evaluate_request,
    }

server = DebugServer('localhost', 4711)
asyncore.loop()
