import logging
import traceback
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
        try:
            self.thread_proc()
        except Exception as e:
            tb = traceback.format_exc(e)
            log.error('Unhandled error on a worker thread: %s', tb)

    def thread_proc():
        pass