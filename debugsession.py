import debugserver
import lldb
from six import print_

class DebugSession(debugserver.SessionHandler):
    debugger = None
    target = None
    process = None

    def __init__(self, sock):
        debugserver.SessionHandler.__init__(self, sock=sock)
        self.debugger = lldb.debugger

    def initialize_request(self, args):
        pass

    def launch_request(self, args):
        self.target = self.debugger.CreateTargetWithFileAndArch(str(args["program"]), lldb.LLDB_ARCH_DEFAULT)
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
        self.process = self.target.LaunchSimple([], None. None)
        broadcaster = self.process.GetBroadcaster()
        listener = lldb.SBListener('DebugSession')
        rc = broadcaster.AddListener(listener, lldb.SBProcess.eBroadcastBitStateChanged)

    def disconnect_request(self, args):
        self.process.Kill()
