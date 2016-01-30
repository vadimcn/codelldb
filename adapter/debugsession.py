import sys
import lldb
import logging
import debugevents
import itertools
import handles
import terminal
import subprocess
import traceback

log = logging.getLogger(__name__)

class DebugSession:

    def __init__(self, event_loop, send_message):
        DebugSession.current = self
        self.event_loop = event_loop
        self.send_message = send_message
        self.debugger = lldb.SBDebugger.Create()
        self.debugger.SetAsync(True)
        self.event_listener = lldb.SBListener('DebugSession')
        self.listener_handler = debugevents.AsyncListener(self.event_listener,
            lambda event: event_loop.dispatch1(self.on_target_event, event))
        self.var_refs = handles.Handles()
        self.breakpoints = dict() # {file => {line => SBBreakpoint}}
        self.threads = set()
        self.terminal = None
        self.handle_request = lambda msg: event_loop.dispatch1(self.on_request, msg)
        # register the 'allthreads' command
        self.debugger.HandleCommand('script import adapter')
        self.debugger.HandleCommand('command script add -f adapter.debugsession.allthreads_command allthreads')

    def on_request(self, request):
        command =  request['command']
        args = request.get('arguments', None)
        log.debug('### %s ###', command)

        response = {
            'type': 'response',
            'command': command,
            'request_seq': request['seq'],
            'success': False,
        }

        handler = getattr(self, command + '_request', None)
        if handler is not None:
            try:
                response['body'] = handler(args)
                response['success'] = True
            except Exception as e:
                tb = traceback.format_exc(e)
                log.error('Internal error:\n' + tb)
                response['success'] = False
                response['body'] = {
                    'error': {
                        'id': 1,
                        'format': 'Internal error: ' + str(e),
                        'showUser': True
                    }
                }
        else:
            log.warning('No handler for %s', command)

        self.send_message(response)

    def on_target_event(self, event):
        if lldb.SBProcess.EventIsProcessEvent(event):
            ev_type = event.GetType()
            if ev_type == lldb.SBProcess.eBroadcastBitStateChanged:
                state = lldb.SBProcess.GetStateFromEvent(event)
                if state == lldb.eStateStopped:
                    if not lldb.SBProcess.GetRestartedFromEvent(event):
                        self.notify_target_stopped(event)
                elif state == lldb.eStateExited:
                    self.send_event('exited', { 'exitCode': self.process.GetExitStatus() })
                    self.send_event('terminated', {}) # TODO: VSCode doesn't seem to handle 'exited' for now
                elif state in [lldb.eStateCrashed, lldb.eStateDetached]:
                    self.send_event('terminated', {})
            elif ev_type & (lldb.SBProcess.eBroadcastBitSTDOUT | lldb.SBProcess.eBroadcastBitSTDERR) != 0:
                self.notify_stdio(ev_type)

    def notify_target_stopped(self, event):
        self.notify_live_threads()

        # VSCode bug #40: On one hand VSCode won't display stacks of the thread that were not reported 'stopped',
        # on the other hand, if we report more than one, the UI will choose which one is the current one
        # seemingly randomly.  Which makes breakpoint stops confusing and stepping - nearly unusable.
        # So here's the compromise: if this stop is due to hitting a breakpoint or stepping, we report only the
        # thread that has caused it.  The user can still display all stacks via the 'allthreads' command.
        # For other stops we report all threads, as the current selection is presumably irrelevant.
        for thread in self.process:
            stop_reason = thread.GetStopReason()
            if stop_reason == lldb.eStopReasonBreakpoint:
                self.send_event('stopped', { 'reason': 'breakpoint', 'threadId': thread.GetThreadID() })
                return
            elif stop_reason in [lldb.eStopReasonTrace, lldb.eStopReasonPlanComplete]:
                self.send_event('stopped', { 'reason': 'step', 'threadId': thread.GetThreadID() })
                return
        # otherwise, report all threads
        for thread in self.process:
            self.send_event('stopped', { 'reason': 'pause', 'threadId': thread.GetThreadID() })

    def notify_stdio(self, ev_type):
        if ev_type == lldb.SBProcess.eBroadcastBitSTDOUT:
            read_stream = self.process.GetSTDOUT
            category = 'stdout'
        else:
            read_stream = self.process.GetSTDERR
            category = 'stderr'
        output = read_stream(1024)
        while output:
            self.send_event('output', { 'category': category, 'output': output })
            output = read_stream(1024)

    def notify_live_threads(self):
        curr_threads = set((thread.GetThreadID() for thread in self.process))
        for tid in self.threads - curr_threads:
            self.send_event('thread', { 'reason': 'exited', 'threadId': tid })
        for tid in curr_threads - self.threads:
            self.send_event('thread', { 'reason': 'started', 'threadId': tid })
        self.threads = curr_threads

    def send_allthreads_stop(self):
        for thread in self.process.threads:
            self.send_event('stopped', { 'reason': 'none', 'threadId': thread.GetThreadID() })

    def send_event(self, event, body):
        message = {
            'type': 'event',
            'seq': 0,
            'event': event,
            'body': body
        }
        self.send_message(message)

    def initialize_request(self, args):
        self.line_offset = 0 if args.get('linesStartAt1', True) else 1
        self.col_offset = 0 if args.get('columnsStartAt1', True) else 1
        return { 'supportsConfigurationDoneRequest': True,
                 'supportsEvaluateForHovers': True,
                 'supportsFunctionBreakpoints': False } # TODO: what are those?

    def launch_request(self, args):
        self.exec_commands(args.get('initCommands'))
        self.target = self.debugger.CreateTargetWithFileAndArch(str(args['program']), lldb.LLDB_ARCH_DEFAULT)
        self.send_event('initialized', {})
        # defer actual launching till the setExceptionBreakpoints request,
        # so that we could set initial breakpoints before the target starts running
        self.do_launch = lambda: self.launch(args)

    def launch(self, args):
        self.exec_commands(args.get('preRunCommands'))
        flags = 0
        # argumetns
        target_args = args.get('args', None)
        if target_args is not None:
            target_args = [str(arg) for arg in target_args]
        # environment
        env = args.get('env', None)
        envp = None
        if (env is not None): # Convert dict to a list of 'key=value' strings
            envp = ['%s=%s' % item for item in env.iteritems()]
        # stdio
        stdio = args.get('stdio', None)
        missing = () # None is a valid value here, so we need a new one to designate 'missing'
        if type(stdio) is dict:
            stdio = [stdio.get('stdin', missing),
                     stdio.get('stdout', missing),
                     stdio.get('stderr', missing)]
        elif type(stdio) in [type(None), str, unicode]:
            stdio = [stdio] * 3
        elif type(stdio) is list:
            stdio.extend([missing] * (3-len(stdio))) # pad up to 3 items
        else:
            raise Exception('stdio must be either a string, a list or an object')
        # replace all missing's with the previous stream's value
        for i in range(0, len(stdio)):
            if stdio[i] == missing:
                stdio[i] = stdio[i-1] if i > 0 else None
        stdio = map(opt_str, stdio) # convert unicode strings to ascii
        # open a new terminal window if needed
        if '*' in stdio:
            if 'linux' in sys.platform:
                self.terminal = terminal.create()
                stdio = [self.terminal.tty if s == '*' else s for s in stdio]
            else:
                # OSX LLDB supports this natively.
                # On Windows LLDB always creates new console window (even if stdio is redirected).
                flags |= lldb.eLaunchFlagLaunchInTTY | lldb.eLaunchFlagCloseTTYOnExit
                stdio = [None if s == '*' else s for s in stdio]
        # working directory
        work_dir = opt_str(args.get('cwd', None))
        stop_on_entry = args.get('stopOnEntry', False)
        # launch!
        error = lldb.SBError()
        self.process = self.target.Launch(self.event_listener,
            target_args, envp, stdio[0], stdio[1], stdio[2],
            work_dir, flags, stop_on_entry, error)
        if not error.Success():
            self.send_event('output', { 'category': 'console', 'output': error.GetCString() })
            self.send_event('terminated', {})
            raise Exception('Process attach failed.')
        assert self.process.IsValid()

    def attach_request(self, args):
        self.exec_commands(args.get('initCommands'))
        self.target = self.debugger.CreateTargetWithFileAndArch(str(args['program']), lldb.LLDB_ARCH_DEFAULT)
        self.send_event('initialized', {})
        self.do_launch = lambda: self.attach(args)

    def attach(self, args):
        self.exec_commands(args.get('preRunCommands'))

        error = lldb.SBError()
        if args.get('pid', None) is not None:
            self.process = self.target.AttachToProcessWithID(self.event_listener, args['pid'], error)
        else:
            self.process = self.target.AttachToProcessWithName(self.event_listener, str(args['program']), True, error)

        if not error.Success():
            self.send_event('output', { 'category': 'console', 'output': error.GetCString() })
            self.send_event('terminated', {})
            raise Exception('Process attach failed.')
        assert self.process.IsValid()

    def exec_commands(self, commands):
        if commands is not None:
            interp = self.debugger.GetCommandInterpreter()
            result = lldb.SBCommandReturnObject()
            for command in commands:
                interp.HandleCommand(str(command), result)
                output = result.GetOutput() if result.Succeeded() else result.GetError()
                self.send_event('output', { 'category': 'console', 'output': output })

    def setBreakpoints_request(self, args):
        file = str(args['source']['path'])
        # The setBreakpoints request is not incremental, it replaces all breakpoints in a file,
        # which is wasteful if all you needed was to add or remove one breakpoint.
        # Therefore, we perform a diff of the request and the existing debugger breakpoints:

        bp_reqs = args['breakpoints']
        bp_lines = [bp_req['line'] for bp_req in bp_reqs]
        # First, we delete existing breakpoints which are not in the new set.
        file_bps = self.breakpoints.setdefault(file, {})
        for line, bp in list(file_bps.items()):
            if line not in bp_lines:
                self.target.BreakpointDelete(bp.GetID())
                del file_bps[line]

        # Next, create breakpoints which were not in the old set
        result = []
        for bp_req in bp_reqs:
            line = bp_req['line']
            bp = file_bps.get(line, None)
            if bp is None:
                bp = self.target.BreakpointCreateByLocation(file, line)
                file_bps[line] = bp
            cond = opt_str(bp_req.get('condition', None))
            if cond != bp.GetCondition():
                bp.SetCondition(cond)
            bp_resp = {
                'id': bp.GetID(),
                'verified': bp.num_locations > 0,
                'line': line # TODO: find out the the actual line
            }
            result.append(bp_resp)

        return { 'breakpoints': result }

    def setExceptionBreakpoints_request(self, args):
        #self.do_launch()
        pass

    def configurationDone_request(self, args):
        self.do_launch()

    def pause_request(self, args):
        self.process.Stop()

    def continue_request(self, args):
        # variable handles will be invalid after running,
        # so we may as well clean them up now
        self.var_refs.reset()
        self.process.Continue()

    def next_request(self, args):
        self.var_refs.reset()
        tid = args['threadId']
        self.process.GetThreadByID(tid).StepOver()

    def stepIn_request(self, args):
        self.var_refs.reset()
        tid = args['threadId']
        self.process.GetThreadByID(tid).StepInto()

    def stepOut_request(self, args):
        self.var_refs.reset()
        tid = args['threadId']
        self.process.GetThreadByID(tid).StepOut()

    def threads_request(self, args):
        threads = []
        for thread in self.process:
            tid = thread.GetThreadID()
            threads.append({ 'id': tid, 'name': '%s:%d' % (thread.GetName(), tid) })
        return { 'threads': threads }

    def stackTrace_request(self, args):
        thread = self.process.GetThreadByID(args['threadId'])
        levels = args.get('levels', 0)
        stack_frames = []
        for i, frame in zip(itertools.count(), thread):
            if levels > 0 and i > levels:
                break

            stack_frame = { 'id': self.var_refs.create(frame) }

            fn = frame.GetFunction()
            if fn.IsValid():
                stack_frame['name'] = fn.GetName()
            else:
                sym = frame.GetSymbol()
                if sym.IsValid():
                    stack_frame['name'] = sym.GetName()
                else:
                    stack_frame['name'] = str(frame.GetPCAddress())

            le = frame.GetLineEntry()
            if le.IsValid():
                fs = le.GetFileSpec()
                stack_frame['source'] = { 'name': fs.GetFilename(), 'path': str(fs) }
                stack_frame['line'] = le.GetLine()
                stack_frame['column'] = le.GetColumn()

            stack_frames.append(stack_frame)

        return { 'stackFrames': stack_frames }

    def scopes_request(self, args):
        locals = { 'name': 'Locals', 'variablesReference': args['frameId'], 'expensive': False }
        return { 'scopes': [locals] }

    def variables_request(self, args):
        variables = []
        obj = self.var_refs.get(args['variablesReference'])
        if obj is None:
            raise Exception('Invalid variable reference')

        if type(obj) is lldb.SBFrame:
            vars = obj.GetVariables(True, True, False, True)
        elif type(obj) is lldb.SBValue:
            vars = obj
        else:
            vars = obj[1].GetNonSyntheticValue()

        for var in vars:
            name, value, dtype, ref = self.parse_var(var)
            variable = { 'name': name, 'value': value, 'type': dtype, 'variablesReference': ref }
            variables.append(variable)

        if type(vars) is lldb.SBValue and vars.IsSynthetic():
            ref = self.var_refs.create(('synthetic', vars))
            variable = { 'name': '[raw]', 'value': vars.GetTypeName(), 'variablesReference': ref }
            variables.append(variable)

        return { 'variables': variables }

    def evaluate_request(self, args):
        context = args['context']
        expr = str(args['expression'])
        if context != 'repl': # i.e. 'watch' or 'hover'
            return self.evaluate_expr(args, expr)
        elif expr.startswith('?'): # "?<expr>" in 'repl' context
            return self.evaluate_expr(args, expr[1:])
        # Else evaluate as debugger command

        # set up evaluation context
        frame = self.var_refs.get(args.get('frameId', None), None)
        if frame is not None:
            thread = frame.GetThread()
            self.process.SetSelectedThread(thread)
            thread.SetSelectedFrame(frame.GetFrameID())
        # evaluate
        interp = self.debugger.GetCommandInterpreter()
        result = lldb.SBCommandReturnObject()
        interp.HandleCommand(str(expr), result)
        output = result.GetOutput() if result.Succeeded() else result.GetError()
        # returning output as result would display all line breaks as '\n'
        self.send_event('output', { 'category': 'console', 'output': output })
        return { 'result': '' }

    def evaluate_expr(self, args, expr):
        frame = self.var_refs.get(args.get('frameId', 0), None)
        if frame is None:
            return
        var = frame.EvaluateExpression(expr)
        if var.GetError().Success():
            _, value, dtype, ref = self.parse_var(var)
            return { 'result': value, 'type': dtype, 'variablesReference': ref }
        else:
            output = var.GetError().GetCString()
            self.send_event('output', { 'category': 'console', 'output': output })

    def parse_var(self, var):
        name = var.GetName()
        value = var.GetValue()
        if value is None:
            value = var.GetSummary()
            if value is not None:
                value = value.replace('\n', '') # VSCode won't display line breaks
        if value is None:
            value = '{...}'
        value = value.decode('latin1') # or else json will try to treat it as utf8
        dtype = var.GetTypeName()
        ref = self.var_refs.create(var) if var.MightHaveChildren() else 0
        return name, value, dtype, ref

    def disconnect_request(self, args):
        self.process.Kill()
        self.terminal = None
        self.event_loop.stop()


def allthreads_command(self, debugger, command, result, internal_dict):
    DebugSession.current.send_allthreads_stop()

def opt_str(s):
    return str(s) if s != None else None
