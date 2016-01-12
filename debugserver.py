import asyncore
import asynchat
import socket
import string
import json
from six import print_

class DebugServer(asyncore.dispatcher):

    def __init__(self, host, port, handler_factory):
        asyncore.dispatcher.__init__(self)
        self.handler_factory = handler_factory
        self.create_socket(socket.AF_INET, socket.SOCK_STREAM)
        self.set_reuse_addr()
        self.bind((host, port))
        self.listen(5)

    def handle_accept(self):
        pair = self.accept()
        if pair is not None:
            sock, addr = pair
            print_("Incoming connection from %s" % repr(addr))
            handler = self.handler_factory(sock)


class SessionHandler(asynchat.async_chat):

    def __init__(self, sock):
        asynchat.async_chat.__init__(self, sock=sock)
        self.ibuffer = []
        self.set_terminator("\r\n\r\n")
        self.reading_headers = True

    def collect_incoming_data(self, data):
        self.ibuffer.append(data)

    def found_terminator(self):
        if self.reading_headers:
            clen = None
            for line in string.split("".join(self.ibuffer), "\r\n"):
                if line.startswith("Content-Length:"):
                    clen = int(string.strip(line[15:]))
            if clen != None:
                self.reading_headers = False
                self.ibuffer = []
                self.set_terminator(clen)
            else:
                print_("ERROR: No Content-Length header")
        else:
            data = "".join(self.ibuffer)
            print_("->", data)
            request = json.loads(data)
            self.dispatch(request)
            self.reading_headers = True
            self.ibuffer = []
            self.set_terminator("\r\n\r\n")

    def dispatch(self, request):
        command =  request["command"]
        args = request["arguments"]
        print_("###", command)

        response = {
            "type": "response",
            "command": command,
            "request_seq": request["seq"],
            "success": False,
        }

        handler = getattr(self, command + "_request", None)
        if handler != None:
            response["body"] = handler(args)
            response["success"] = True
        else:
            print_("(No handler for", command,")")

        data = json.dumps(response)
        self._send(data)

    def send_event(self, event, body):
        message = {
            "type": "event",
            "seq": 0,
            "event": event,
            "body": body
        }
        data = json.dumps(message)
        self._send(data)

    def _send(self, data):
        print_("<-", data)
        self.push("Content-Length: %d\r\n\r\n" % len(data))
        self.push(data)
