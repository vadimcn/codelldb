import lldb
import threading
import logging

log = logging.getLogger(__name__)

class AsyncListener:
    def __init__(self, listener, event_sink):
        assert listener.IsValid()
        self.listener = listener
        self.event_sink = event_sink

        self.stopping = False
        self.read_thread = threading.Thread(None, self.pump_events)
        self.read_thread.start()

    def __del__(self):
        self.stopping = True
        self.thread.join()

    def pump_events(self):
        event = lldb.SBEvent()
        while not self.stopping:
            if self.listener.WaitForEvent(1, event):
                if log.isEnabledFor(logging.DEBUG):
                    descr = lldb.SBStream()
                    event.GetDescription(descr)
                    log.debug('Event: %s %s', event.GetDataFlavor(), descr.GetData())
                self.event_sink(event)
                event = lldb.SBEvent()
