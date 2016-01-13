import lldb
import logging
import debugevents
import itertools
import handles
import util

log = logging.getLogger(__name__)

def opt_str(s):
    return str(s) if s != None else None

class DebugSession:

    def __init__(self, event_loop, send_message):
        self.event_loop = event_loop
        self.send_message = send_message
        self.debugger = lldb.debugger
        self.debugger.SetAsync(True)
        self.event_listener = lldb.SBListener("DebugSession")
        def dispatch_event(event):
            event_loop.dispatch1(self.on_target_event, event)
        self.listener_handler = debugevents.AsyncListener(self.event_listener, dispatch_event)
        self.var_refs = handles.Handles()

    def on_request(self, request):
        command =  request["command"]
        args = request.get("arguments", None)
        log.info("### %s ###", command)

        response = {
            "type": "response",
            "command": command,
            "request_seq": request["seq"],
            "success": False,
        }

        handler = getattr(self, command + "_request", None)
        if handler != None:
            response["body"] = handler(args)
            response["success"] = True
        else:
            log.warning("No handler for %s", command)

        self.send_message(response)

    def on_target_event(self, event):
        util.print_event(event)

        if lldb.SBProcess.EventIsProcessEvent(event):
            type = event.GetType()
            if type == lldb.SBProcess.eBroadcastBitStateChanged:
                state = lldb.SBProcess.GetStateFromEvent(event)
                if state == lldb.eStateStopped:
                    if all((thread.GetStopReason() == lldb.eStopReasonNone for thread in self.process)):
                        return
                    for thread in self.process:
                        self.send_event("stopped", { "reason": "breakpoint", "threadId": thread.GetThreadID() })

    def send_event(self, event, body):
        message = {
            "type": "event",
            "seq": 0,
            "event": event,
            "body": body
        }
        self.send_message(message)

    def initialize_request(self, args):
        pass

    def launch_request(self, args):
        self.target = self.debugger.CreateTargetWithFileAndArch(str(args["program"]), lldb.LLDB_ARCH_DEFAULT)
        self.launch_args = args
        self.send_event("initialized", {})

    def do_launch(self):
        error = lldb.SBError()
        args = opt_str(self.launch_args.get("args", None))
        env = opt_str(self.launch_args.get("env", None))
        work_dir = opt_str(self.launch_args.get("cwd", None))
        stop_on_entry = self.launch_args.get("stopOnEntry", False)
        flags = 0
        if self.launch_args.get("stdio", None) == "*":
            flags != lldb.eLaunchFlagLaunchInTTY
        self.process = self.target.Launch(self.event_listener,
            args, env, None, None, None, work_dir, flags, stop_on_entry, error)
        assert self.process.IsValid()

    def setBreakpoints_request(self, args):
        file = str(args["source"]["path"])
        breakpoints = []
        for line in args["lines"]:
            bp = self.target.BreakpointCreateByLocation(file, line)
            breakpoints.append({
                "verified": bp.num_locations > 0,
                "line": line
            })
        return { "breakpoints": breakpoints }

    def setExceptionBreakpoints_request(self, args):
        self.do_launch()

    def pause_request(self, args):
        self.process.Stop()

    def continue_request(self, args):
        self.process.Continue()

    def threads_request(self, args):
        threads = []
        for thread in self.process:
            threads.append({ "id": thread.GetThreadID(),
                             "name": "%s:%d" % (thread.GetName(), thread.GetThreadID()) })
        return { "threads": threads }

    def stackTrace_request(self, args):
        thread = self.process.GetThreadByID(args["threadId"])
        levels = args.get("levels", 0)
        stack_frames = []
        for i, frame in zip(itertools.count(), thread):
            if levels > 0 and i > levels:
                break

            stack_frame = { "id": self.var_refs.create(frame) }

            fn = frame.GetFunction()
            if fn.IsValid():
                stack_frame["name"] = fn.GetName()
            else:
                sym = frame.GetSymbol()
                if sym.IsValid():
                    stack_frame["name"] = sym.GetName()
                else:
                    stack_frame["name"] = str(frame.GetPCAddress())

            le = frame.GetLineEntry()
            if le.IsValid():
                fs = le.GetFileSpec()
                stack_frame["source"] = { "name": fs.GetFilename(), "path": str(fs) }
                stack_frame["line"] = le.GetLine()
                stack_frame["column"] = le.GetColumn()

            stack_frames.append(stack_frame)

        return { "stackFrames": stack_frames }

    def scopes_request(self, args):
        locals = { "name": "Locals", "variablesReference": args["frameId"], "expensive": False }
        return { "scopes": [locals] }

    def variables_request(self, args):
        variables = []
        obj = self.var_refs.get(args["variablesReference"])

        if type(obj) is lldb.SBFrame:
            vars = obj.GetVariables(True, True, False, True)
        elif type(obj) is lldb.SBValue:
            vars = obj

        for var in vars:
            name = var.GetName()
            value = var.GetValue()
            if value is None:
                value = "{...}"
            ref = self.var_refs.create(var) if var.MightHaveChildren() else 0

            variable = { "name": name, "value": value, "variablesReference": ref }
            variables.append(variable)

        return { "variables": variables }

    def evaluate_request(self, args):
        if args["context"] == "repl":
            command = args["expression"]
            if command == "test":
                self.target.BreakpointCreateByLocation("/usr/local/google/home/vadimcn/NW/vscode-lldb/debuggee/src/main.rs", 25)
                return

            interp = self.debugger.GetCommandInterpreter()
            result = lldb.SBCommandReturnObject()
            interp.HandleCommand(str(command), result)
            output = result.GetOutput() if result.Succeeded() else result.GetError()
            self.send_event("output", { "category": "console", "output": output })
            return { "result": "" }

    def disconnect_request(self, args):
        self.process.Kill()
        self.event_loop.stop()
