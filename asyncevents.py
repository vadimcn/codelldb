import asyncore
import socket
import errno
import Queue
import threading
import lldb

class ListenerThread(asyncore.dispatcher):
    """
        This class abstracts a lldb.SBListener polling thread, which
        marshals received events to asyncore's loop thread and invokes
        the event sink function there.
        Unfortunately, socket events are the only thing that can wake up
        asyncore :(
    """

    def __init__(self, listener, event_sink):
        """
            listener: SBListener
            event_sink: callable(SBEvent)
        """
        self.listener = listener
        self.event_sink = event_sink

        self.queue = Queue.Queue()

        s1, s2 = self._make_socket_pair()
        asyncore.dispatcher.__init__(self, s1)
        self.write_socket = s2

        self.shutting_down = False
        self.thread = threading.Thread(None, self._thread_proc)
        self.thread.start()

    def __del__(self):
        self.shutting_down = True
        self.thread.join()

    def _make_socket_pair(self):
        # See if socketpair() is available.
        have_socketpair = hasattr(socket, 'socketpair')
        if have_socketpair:
            client_sock, srv_sock = socket.socketpair()
            return client_sock, srv_sock

        # Create a non-blocking temporary server socket
        temp_srv_sock = socket.socket()
        temp_srv_sock.setblocking(False)
        temp_srv_sock.bind(('127.0.0.1', 0))
        port = temp_srv_sock.getsockname()[1]
        temp_srv_sock.listen(1)

        # Create non-blocking client socket
        client_sock = socket.socket()
        client_sock.setblocking(False)
        try:
            client_sock.connect(('127.0.0.1', port))
        except socket.error as err:
            # EWOULDBLOCK is not an error, as the socket is non-blocking
            if err.errno not in [errno.EWOULDBLOCK, errno.WSAEWOULDBLOCK]:
                raise

        # Wait for connect
        import select
        timeout = 1
        readable = select.select([temp_srv_sock], [], [], timeout)[0]
        if temp_srv_sock not in readable:
            raise Exception('Client socket not connected in {} second(s)'.format(timeout))
        srv_sock, _ = temp_srv_sock.accept()
        return client_sock, srv_sock

    def _thread_proc(self):
        event = lldb.SBEvent()
        while not self.shutting_down:
            if self.listener.WaitForEvent(1, event):
                self.queue.put(event)
                self.write_socket.send("#")
                event = lldb.SBEvent()

    def handle_read(self):
        x = self.recv(1)
        event = self.queue.get()
        self.event_sink(event)
