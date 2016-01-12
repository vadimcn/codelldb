import debugserver
import lldb
import asyncevents
from six import print_

class DebugSession(debugserver.SessionHandler):
    debugger = None
    target = None
    process = None
    event_marshaler = None

    def __init__(self, sock):
        debugserver.SessionHandler.__init__(self, sock=sock)
        self.debugger = lldb.debugger
        self.debugger.SetAsync(True)

    def on_target_event(self, event):
        stm = lldb.SBStream()
        event.GetDescription(stm)
        state = lldb.SBProcess.GetStateFromEvent(event)
        print_("$$$$$", state, stm.GetData())

    def initialize_request(self, args):
        pass

    def launch_request(self, args):
        self.target = self.debugger.CreateTargetWithFileAndArch(str(args["program"]), lldb.LLDB_ARCH_DEFAULT)
        broadcaster = self.target.GetBroadcaster()
        listener = lldb.SBListener('DebugSession')
        rc = broadcaster.AddListener(listener, lldb.SBProcess.eBroadcastBitStateChanged | lldb.SBProcess.eBroadcastBitInterrupt)
        self.event_marshaler = asyncevents.ListenerThread(listener, self.on_target_event)
        self.send_event("initialized", {})

    def setBreakpoints_request(self, args):
        file = lldb.SBFileSpec(str(args["source"]), True)
        breakpoints = []
        for line in args["lines"]:
            bp = self.target.BreakpointCreateByLocation(file, line)
            breakpoints.append({
                "verified": bp.IsValid(),
                "line": line
            })
        return { "breakpoints": breakpoints }

    def setExceptionBreakpoints_request(self, args):
        self.process = self.target.LaunchSimple([], None, None)

    def pause_request(self, args):
        self.process.Stop()

    def disconnect_request(self, args):
        self.process.Kill()
        self.close_when_done()
