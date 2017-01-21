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
        self.daemon = True
        self.stopping = False

    def start(self):
        Thread.start(self)
        # Automatically calls shutdown() when the returned token goes out of scope
        return ScopeGuard(self.shutdown)

    def shutdown(self):
        log.debug('%s thread is shutting down', self.__class__.__name__)
        self.stopping = True
        self.join()
        log.debug('%s thread has stopped', self.__class__.__name__)

    def run(self):
        try:
            self.thread_proc()
        except Exception as e:
            log.error('%s', traceback.format_exc())

    def thread_proc():
        pass
