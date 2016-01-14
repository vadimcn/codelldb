import sys
import lldb
import logging
import debugevents
import itertools
import handles
import terminal
import subprocess
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
        self.event_listener = lldb.SBListener('DebugSession')
        self.listener_handler = debugevents.AsyncListener(self.event_listener,
            lambda event: event_loop.dispatch1(self.on_target_event, event))
        self.var_refs = handles.Handles()
        self.terminal = None
        self.handle_request = lambda msg: event_loop.dispatch1(self.on_request, msg)

    def on_request(self, request):
        command =  request['command']
        args = request.get('arguments', None)
        log.info('### %s ###', command)

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
                log.error(repr(e))
                response['success'] = False
                response['body'] = {
                    'error': {
                        'id': 1,
                        'format': str(e),
                        'showUser': True
                    }
                }
        else:
            log.warning('No handler for %s', command)

        self.send_message(response)

    def on_target_event(self, event):
        util.print_event(event)

        if lldb.SBProcess.EventIsProcessEvent(event):
            type = event.GetType()
            if type == lldb.SBProcess.eBroadcastBitStateChanged:
                state = lldb.SBProcess.GetStateFromEvent(event)

                if state == lldb.eStateStopped:
                    self.notify_target_stopped()
                elif state == lldb.eStateExited:
                    self.send_event('exited', { 'exitCode': self.process.GetExitStatus() })
                    self.send_event('terminated', {}) # VSCode doesn't seem to handle 'exited' for now
                elif state in [lldb.eStateCrashed, lldb.eStateDetached]:
                    self.send_event('terminated', {})

    def notify_target_stopped(self):
        if all((thread.GetStopReason() == lldb.eStopReasonNone for thread in self.process)):
            return

        causing_thread = None
        for thread in self.process:
            if thread.GetStopReason() != lldb.eStopReasonNone:
                causing_thread = thread
            else:
                self.send_event('stopped', { 'reason': 'none', 'threadId': thread.GetThreadID() })

        self.send_event('stopped', { 'reason': 'breakpoint', 'threadId': causing_thread.GetThreadID() })


    def send_event(self, event, body):
        message = {
            'type': 'event',
            'seq': 0,
            'event': event,
            'body': body
        }
        self.send_message(message)

    def initialize_request(self, args):
        pass

    def launch_request(self, args):
        self.target = self.debugger.CreateTargetWithFileAndArch(str(args['program']), lldb.LLDB_ARCH_DEFAULT)
        self.send_event('initialized', {})
        # defer actual launching till the setExceptionBreakpoints request,
        # so that we could set initial breakpoints before the target starts running
        self.do_launch = lambda: self.launch(args)

    def launch(self, args):
        flags = 0

        target_args = args.get('args', None)
        if target_args is not None:
            target_args = [str(arg) for arg in args]

        env = args.get('env', None)
        envp = None
        if (env is not None): # Convert dict to a list of 'key=value' strings
            envp = ['%s=%s' % item for item in env.iteritems()]

        stdio = opt_str(args.get('stdio', None))
        if stdio == '*':
            if sys.platform == 'darwin':
                # OSX supports this natively
                flags |= lldb.eLaunchFlagLaunchInTTY | lldb.eLaunchFlagCloseTTYOnExit
            else:
                self.terminal = terminal.create()
                stdio = self.terminal.tty

        work_dir = opt_str(args.get('cwd', None))
        stop_on_entry = args.get('stopOnEntry', False)
        error = lldb.SBError()
        self.process = self.target.Launch(self.event_listener,
            target_args, envp, stdio, stdio, stdio, work_dir, flags, stop_on_entry, error)
        assert self.process.IsValid(), 'process.IsValid()'

    def attach_request(self, args):
        self.target = self.debugger.CreateTargetWithFileAndArch(str(args['program']), lldb.LLDB_ARCH_DEFAULT)
        self.send_event('initialized', {})
        self.do_launch = lambda: self.attach(args)

    def attach(self, args):
        error = lldb.SBError()
        if args.get('pid', None) is not None:
            self.process = self.target.AttachToProcessWithID(self.event_listener, args['pid'], error)
        else:
            self.process = self.target.AttachToProcessWithName(self.event_listener, str(args['program']), True, error)

        if not error.Success():
            raise Exception(error.GetCString())
        assert self.process.IsValid(), 'process.IsValid()'

    def setBreakpoints_request(self, args):
        file = str(args['source']['path'])
        breakpoints = []
        for line in args['lines']:
            bp = self.target.BreakpointCreateByLocation(file, line)
            breakpoints.append({
                'verified': bp.num_locations > 0,
                'line': line
            })
        return { 'breakpoints': breakpoints }

    def setExceptionBreakpoints_request(self, args):
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
        self.process.GetSelectedThread().StepOver()

    def stepIn_request(self, args):
        self.var_refs.reset()
        self.process.GetSelectedThread().StepInto()

    def stepOut_request(self, args):
        self.var_refs.reset()
        self.process.GetSelectedThread().StepOut()

    def threads_request(self, args):
        threads = []
        for thread in self.process:
            threads.append({ 'id': thread.GetThreadID(),
                             'name': '%s:%d' % (thread.GetName(), thread.GetThreadID()) })
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

        if type(obj) is lldb.SBFrame:
            vars = obj.GetVariables(True, True, False, True)
        elif type(obj) is lldb.SBValue:
            vars = obj

        for var in vars:
            name, value, dtype, ref = self.parse_var(var)
            variable = { 'name': name, 'value': value, 'type': dtype, 'variablesReference': ref }
            variables.append(variable)

        return { 'variables': variables }

    def evaluate_request(self, args):
        context = args['context']
        expr = str(args['expression'])
        if context == 'watch':
            return self.evaluate_expr(args, expr)
        elif expr.startswith('?'): # 'repl'
            return self.evaluate_expr(args, expr[1:])
        # evaluate as debugger command
        interp = self.debugger.GetCommandInterpreter()
        result = lldb.SBCommandReturnObject()
        interp.HandleCommand(str(expr), result)
        output = result.GetOutput() if result.Succeeded() else result.GetError()
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
        if value is None:
            value = '{...}'
        dtype = var.GetTypeName()
        ref = self.var_refs.create(var) if var.MightHaveChildren() else 0
        return (name, value, dtype, ref)

    def disconnect_request(self, args):
        self.process.Kill()
        self.terminal = None
        self.event_loop.stop()
