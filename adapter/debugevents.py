import threading
import logging

log = logging.getLogger(__name__)

class ReaderThread(threading.Thread):
    def __init__(self, *args):
        threading.Thread.__init__(self, target = self.thread_proc, args = args)
        self.stopping = False
        self.start()

    def __del__(self):
        self.stopping = True
        self.join()

class AsyncListener(ReaderThread):
    def thread_proc(self, listener, event_sink):
        import lldb
        event = lldb.SBEvent()
        while not self.stopping:
            if listener.WaitForEvent(1, event):
                if log.isEnabledFor(logging.DEBUG):
                    descr = lldb.SBStream()
                    event.GetDescription(descr)
                    log.debug('### Debug event: %s %s', event.GetDataFlavor(), descr.GetData())
                event_sink(event)
                event = lldb.SBEvent()

class PipeReader(ReaderThread):
    def thread_proc(self, pipe, data_sink):
        while not self.stopping:
            data = pipe.read(256)
            if len(data) == 0:
                break
            data_sink(data)
