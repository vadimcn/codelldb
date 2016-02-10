import sys
if sys.version_info[0] > 2: import queue
else: import Queue as queue

class EventLoop:
    def __init__(self, qsize=10):
        self.queue = queue.Queue(qsize)

    def dispatch(self, target, args):
        '''Dispatch to function with a tuple of arguments'''
        self.queue.put((target, args))

    def dispatch1(self, target, arg):
        '''Dispatch to function with 1 argument'''
        self.queue.put((target, (arg,)))

    def run(self):
        self.stopping = False
        while not self.stopping:
            target, args = self.queue.get()
            target(*args)

    def stop(self):
        self.stopping = True