import sys
import logging
import os.path
import itertools
import subprocess
import traceback
import lldb
from . import debugevents
from . import handles
from . import terminal
from . import PY2

log = logging.getLogger(__name__)

class DebugSession:

    def __init__(self, event_loop, send_message):
        DebugSession.current = self
        self.event_loop = event_loop
        self.send_message = send_message
        self.var_refs = handles.Handles()
        self.breakpoints = dict() # { file : { line : SBBreakpoint } }
        self.fn_breakpoints = dict() # { name : SBBreakpoint }
        self.exc_breakpoints = []
        self.target = None
        self.process = None
        self.threads = set()
        self.terminal = None
        self.launch_args = None

    # handles messages from VSCode
    def handle_message(self, msg):
        self.event_loop.dispatch1(self.on_request, msg)

    # handles debugger notifications
    def handle_event(self, event):
        self.event_loop.dispatch1(self.on_target_event, event)

    def initialize_request(self, args):
        self.line_offset = 0 if args.get('linesStartAt1', True) else 1
        self.col_offset = 0 if args.get('columnsStartAt1', True) else 1
        self.debugger = lldb.SBDebugger.Create()
        log.info('LLDB version: %s', self.debugger.GetVersionString())
        self.debugger.SetAsync(True)
        self.event_listener = lldb.SBListener('DebugSession')
        self.listener_handler = debugevents.AsyncListener(self.event_listener, self.handle_event)

        return { 'supportsConfigurationDoneRequest': True,
                 'supportsEvaluateForHovers': True,
                 'supportsFunctionBreakpoints': True,
                 'supportsConditionalBreakpoints': True }

    def launch_request(self, args):
        self.exec_commands(args.get('initCommands'))
        self.target = self.debugger.CreateTargetWithFileAndArch(str(args['program']), lldb.LLDB_ARCH_DEFAULT)
        if not self.target.IsValid():
            raise UserError('Could not initialize debug target (is the program path correct?)')
        self.send_event('initialized', {})
        self.launch_args = args
        # defer actual launching till configurationDone request, so that
        # we can receive and set initial breakpoints before the target starts running
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
            envp = ['%s=%s' % item for item in env.items()]
        # stdio
        stdio = args.get('stdio', None)
        missing = () # None is a valid value here, so we need a new one to designate 'missing'
        if isinstance(stdio, dict):
            stdio = [stdio.get('stdin', missing),
                     stdio.get('stdout', missing),
                     stdio.get('stderr', missing)]
        elif stdio is None or isinstance(stdio, string_type):
            stdio = [stdio] * 3
        elif isinstance(stdio, list):
            stdio.extend([missing] * (3-len(stdio))) # pad up to 3 items
        else:
            raise UserError('stdio must be either a string, a list or an object')
        # replace all missing's with the previous stream's value
        for i in range(0, len(stdio)):
            if stdio[i] == missing:
                stdio[i] = stdio[i-1] if i > 0 else None
        stdio = list(map(opt_str, stdio))
        # open a new terminal window if needed
        if '*' in stdio:
            if 'linux' in sys.platform:
                self.terminal = terminal.create()
                stdio = [self.terminal.tty if s == '*' else s for s in stdio]
            else:
                # OSX LLDB supports this natively.
                # On Windows LLDB always creates a new console window (even if stdio is redirected).
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
            self.console_msg(error.GetCString())
            self.send_event('terminated', {})
            raise UserError('Process launch failed.')
        assert self.process.IsValid()

    def attach_request(self, args):
        self.exec_commands(args.get('initCommands'))
        self.target = self.debugger.CreateTargetWithFileAndArch(str(args['program']), lldb.LLDB_ARCH_DEFAULT)
        if not self.target.IsValid():
            raise UserError('Could not initialize debug target (is the program path correct?)')
        self.send_event('initialized', {})
        self.launch_args = args
        self.do_launch = lambda: self.attach(args)

    def attach(self, args):
        self.exec_commands(args.get('preRunCommands'))

        error = lldb.SBError()
        if args.get('pid', None) is not None:
            self.process = self.target.AttachToProcessWithID(self.event_listener, args['pid'], error)
        else:
            self.process = self.target.AttachToProcessWithName(self.event_listener, str(args['program']), True, error)

        if not error.Success():
            self.console_msg(error.GetCString())
            self.send_event('terminated', {})
            raise UserError('Process attach failed.')
        assert self.process.IsValid()

    def exec_commands(self, commands):
        if commands is not None:
            interp = self.debugger.GetCommandInterpreter()
            result = lldb.SBCommandReturnObject()
            for command in commands:
                interp.HandleCommand(str(command), result)
                output = result.GetOutput() if result.Succeeded() else result.GetError()
                self.console_msg(output)

    def setBreakpoints_request(self, args):
        source = args['source']
        file = str(source['path'])
        req_bps = args['breakpoints']
        req_bp_lines = [req['line'] for req in req_bps]
        # Existing breakpints indexed by line
        curr_bps = self.breakpoints.setdefault(file, {})
        # Existing breakpints that were removed
        for line,bp in list(curr_bps.items()):
            if line not in req_bp_lines:
                self.target.BreakpointDelete(bp.GetID())
                del curr_bps[line]
        # Added or updated
        result = []
        for req in req_bps:
            line = req['line']
            bp = curr_bps.get(line, None)
            if bp is None:
                bp = self.target.BreakpointCreateByLocation(file, line)
                curr_bps[line] = bp
            cond = opt_str(req.get('condition', None))
            if cond != bp.GetCondition():
                bp.SetCondition(cond)
            result.append(self.make_bp_resp(bp))

        return { 'breakpoints': result }

    def setFunctionBreakpoints_request(self, args):
        # Breakpoint requests indexed by function name
        req_bps =  args['breakpoints']
        req_bp_names = [req['name'] for req in req_bps]
        # Existing breakpints that were removed
        for name,bp in list(self.fn_breakpoints.items()):
            if name not in req_bp_names:
                self.target.BreakpointDelete(bp.GetID())
                del self.fn_breakpoints[name]
        # Added or updated
        result = []
        for req in req_bps:
            name = req['name']
            bp = self.fn_breakpoints.get(name, None)
            if bp is None:
                bp = self.target.BreakpointCreateByName(str(name))
                self.fn_breakpoints[name] = bp
            cond = opt_str(req.get('condition', None))
            if cond != bp.GetCondition():
                bp.SetCondition(cond)
            result.append(self.make_bp_resp(bp))

        return { 'breakpoints': result }

    # Create breakpoint location info for a response message
    def make_bp_resp(self, bp):
        if bp.num_locations == 0:
            return { 'id': bp.GetID(), 'verified': False }
        le = bp.GetLocationAtIndex(0).GetAddress().GetLineEntry()
        fs = le.GetFileSpec()
        if not (le.IsValid() and fs.IsValid()):
            return { 'id': bp.GetID(), 'verified': True }
        source = { 'name': fs.basename, 'path': fs.fullpath }
        return { 'id': bp.GetID(), 'verified': True, 'source': source, 'line': le.line }

    def setExceptionBreakpoints_request(self, args):
        filters = args['filters']
        for bp in self.exc_breakpoints:
            self.target.BreakpointDelete(bp.GetID())
        self.exc_breakpoints = []

        source_languages = self.launch_args.get('sourceLanguages', [])
        set_all = 'all' in filters
        set_uncaught = 'uncaught' in filters
        for lang in source_languages:
            bp_setters = DebugSession.lang_exc_bps.get(lang)
            if bp_setters is not None:
                if set_all:
                    bp = bp_setters[0](self.target)
                    self.exc_breakpoints.append(bp)
                if set_uncaught:
                    bp = bp_setters[1](self.target)
                    self.exc_breakpoints.append(bp)

    lang_exc_bps = {
        'rust': (lambda target: target.BreakpointCreateByName('rust_panic'),
                 lambda target: target.BreakpointCreateByName('abort')),
        'cpp': (lambda target: target.BreakpointCreateForException(lldb.eLanguageTypeC_plus_plus, True, False),
                lambda target: target.BreakpointCreateByName('terminate')),
    }

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
        start_frame = args.get('startFrame', 0)
        levels = args.get('levels', sys.maxsize)
        if start_frame + levels > thread.num_frames:
            levels = thread.num_frames - start_frame
        stack_frames = []
        for i in range(start_frame, start_frame + levels):
            frame = thread.frames[i]
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
                # VSCode gets confused if the path contains funky stuff like a double-slash
                full_path = os.path.normpath(fs.fullpath)
                stack_frame['source'] = { 'name': fs.basename, 'path': full_path }
                stack_frame['line'] = le.GetLine()
                stack_frame['column'] = le.GetColumn()
            stack_frames.append(stack_frame)
        return { 'stackFrames': stack_frames, 'totalFrames': len(thread) }

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
        if context in ['watch', 'hover']:
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
        self.console_msg(output)
        return { 'result': '' }

    def evaluate_expr(self, args, expr):
        frame = self.var_refs.get(args.get('frameId', 0), None)
        if frame is None:
            return
        var = frame.EvaluateExpression(expr)
        if var.GetError().Success():
            _, value, dtype, ref = self.parse_var(var)
            return { 'result': value, 'type': dtype, 'variablesReference': ref }
        elif args['context'] != 'hover':
            # don't print errors for hover evals
            output = var.GetError().GetCString()
            self.console_msg(output)

    def parse_var(self, var):
        name = var.GetName()
        value = var.GetValue()
        if value is None:
            value = var.GetSummary()
            if value is not None:
                value = value.replace('\n', '') # VSCode won't display line breaks
        if value is None:
            value = '{...}'
        if PY2:
            value = value.decode('latin1') # or else json will try to treat it as utf8
        dtype = var.GetTypeName()
        ref = self.var_refs.create(var) if var.MightHaveChildren() else 0
        return name, value, dtype, ref

    def disconnect_request(self, args):
        if self.process:
            self.process.Kill()
        self.process = None
        self.target = None
        self.terminal = None
        self.event_loop.stop()

    def on_request(self, request):
        command =  request['command']
        args = request.get('arguments', None)
        log.debug('### Handling command: %s', command)

        response = { 'type': 'response', 'command': command,
                     'request_seq': request['seq'], 'success': False }

        handler = getattr(self, command + '_request', None)
        if handler is not None:
            try:
                response['body'] = handler(args)
                response['success'] = True
            except UserError as e:
                response['success'] = False
                response['body'] = { 'error': { 'id': 0, 'format': str(e), 'showUser': True } }
            except Exception as e:
                tb = traceback.format_exc(e)
                log.error('Internal error:\n' + tb)
                msg = 'Internal error: ' + str(e)
                response['success'] = False
                response['body'] = { 'error': { 'id': 0, 'format': msg, 'showUser': True } }
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
                    exit_code = self.process.GetExitStatus()
                    self.console_msg('Process exited with code %d' % exit_code)
                    self.send_event('exited', { 'exitCode': exit_code })
                    self.send_event('terminated', {}) # TODO: VSCode doesn't seem to handle 'exited' for now
                elif state in [lldb.eStateCrashed, lldb.eStateDetached]:
                    self.send_event('terminated', {})
            elif ev_type & (lldb.SBProcess.eBroadcastBitSTDOUT | lldb.SBProcess.eBroadcastBitSTDERR) != 0:
                self.notify_stdio(ev_type)

    def notify_target_stopped(self, event):
        self.notify_live_threads()
        event = { 'allThreadsStopped': True } # LLDB always stops all threads
        # Find the thread that caused this stop
        for thread in self.process:
            stop_reason = thread.GetStopReason()
            if stop_reason == lldb.eStopReasonBreakpoint:
                event['reason'] = 'breakpoint'
                event['threadId'] = thread.GetThreadID()
                break
            elif stop_reason in [lldb.eStopReasonTrace, lldb.eStopReasonPlanComplete]:
                event['reason'] = 'step'
                event['threadId'] = thread.GetThreadID()
                break
            elif stop_reason == lldb.eStopReasonSignal:
                event['reason'] = 'signal'
                event['threadId'] = thread.GetThreadID()
                break
        else:
            event['reason'] = 'unknown'
        self.send_event('stopped', event)

    def notify_stdio(self, ev_type):
        if ev_type == lldb.SBProcess.eBroadcastBitSTDOUT:
            read_stream = self.process.GetSTDOUT
            category = 'stdout'
        else:
            read_stream = self.process.GetSTDERR
            category = 'stderr'
        output = read_stream(1024)
        while output:
            self.console_msg(output)
            output = read_stream(1024)

    def notify_live_threads(self):
        curr_threads = set((thread.GetThreadID() for thread in self.process))
        for tid in self.threads - curr_threads:
            self.send_event('thread', { 'reason': 'exited', 'threadId': tid })
        for tid in curr_threads - self.threads:
            self.send_event('thread', { 'reason': 'started', 'threadId': tid })
        self.threads = curr_threads

    def send_event(self, event, body):
        message = {
            'type': 'event',
            'seq': 0,
            'event': event,
            'body': body
        }
        self.send_message(message)

    # Write a message to debug console
    def console_msg(self, output):
        self.send_event('output', { 'category': 'console', 'output': output })

# For when we need to let user know they screwed up
class UserError(Exception):
    pass

def opt_str(s):
    return str(s) if s != None else None

string_type = basestring if PY2 else str