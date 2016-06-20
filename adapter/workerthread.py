import logging
from threading import Thread

log = logging.getLogger('workerthread')

class ScopeGuard:
    def __init__(self, target):
        self.target = target

    def __del__(self):
        self.target()

# Thread with a built-in graceful shutdown flag
class WorkerThread(Thread):
    def __init__(self, *args):
        Thread.__init__(self, *args)
        self.stopping = False

    def start(self):
        Thread.start(self)
        # Automatically calls shutdown() when the returned token goes out of scope
        return ScopeGuard(self.shutdown)

    def shutdown(self):
        self.stopping = True
        self.join()

    def run(self):
        while not self.stopping:
            self.run_iteration()
