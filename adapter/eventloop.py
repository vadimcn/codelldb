import logging
from . import PY2
if PY2: import Queue as queue
else: import queue

log = logging.getLogger('eventloop')

class EventLoop:
    def __init__(self, qsize=10):
        self.stopping = False
        self.queue = queue.Queue(qsize)

    # Returns callable object that will dispatch a call to `target`
    # via this event loop's queue.
    def make_dispatcher(self, target):
        def dispatcher(*args):
            if not self.stopping:
                self.queue.put((target, args))
        return dispatcher

    def run(self):
        self.stopping = False
        while not self.stopping:
            target, args = self.queue.get()
            target(*args)

    def stop(self):
        self.stopping = True
