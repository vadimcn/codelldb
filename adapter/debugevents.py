import logging
import lldb
from .workerthread import WorkerThread

log = logging.getLogger('debugevents')

class AsyncListener(WorkerThread):
    def __init__(self, listener, event_sink):
        WorkerThread.__init__(self)
        self.listener = listener
        self.event_sink = event_sink
        self.event = lldb.SBEvent()

    def thread_proc(self):
        while not self.stopping:
            event = self.event
            if self.listener.WaitForEvent(1, event):
                if log.isEnabledFor(logging.DEBUG):
                    descr = lldb.SBStream()
                    event.GetDescription(descr)
                    log.debug('$$$ Debug event: %s %s', event.GetDataFlavor(), descr.GetData())
                self.event_sink(event)
                self.event = lldb.SBEvent()


class DebuggerOutputListener(WorkerThread):
    def __init__(self, stream, event_sink):
        WorkerThread.__init__(self)
        self.stream = stream
        self.event_sink = event_sink
    
    def thread_proc(self):
        while not self.stopping:
            text = self.stream.readline()
            log.debug('$$$ Debug output: %s', text)
            self.event_sink(text)
