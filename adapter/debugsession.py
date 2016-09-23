import sys
import logging
import os.path
import shlex
import traceback
import lldb
from . import debugevents
from . import disassembly
from . import handles
from . import terminal
from . import PY2

log = logging.getLogger('debugsession')
log.info('Imported')

class DebugSession:

    def __init__(self, event_loop, send_message, send_extension_message):
        DebugSession.current = self
        self.event_loop = event_loop
        self.send_message = send_message
        self.send_extension_message = send_extension_message
        self.var_refs = handles.StableHandles()
        self.ignore_bp_events = False
        self.breakpoints = dict() # { file_id : { line : SBBreakpoint } }
        self.fn_breakpoints = dict() # { name : SBBreakpoint }
        self.exc_breakpoints = []
        self.target = None
        self.process = None
        self.terminal = None
        self.launch_args = None
        self.process_launched = False
        self.show_disassembly = 'auto' # never | auto | always
        self.global_format = lldb.eFormatDefault
        self.disassembly_by_handle = handles.Handles()
        self.disassembly_by_addr = []
        self.request_seq = 1
        self.pending_requests = {} # { seq : on_complete }

    def DEBUG_initialize(self, args):
        self.line_offset = 0 if args.get('linesStartAt1', True) else 1
        self.col_offset = 0 if args.get('columnsStartAt1', True) else 1
        self.debugger = lldb.SBDebugger.Create()
        log.info('LLDB version: %s', self.debugger.GetVersionString())
        self.debugger.SetAsync(True)
        self.event_listener = lldb.SBListener('DebugSession')
        listener_handler = debugevents.AsyncListener(self.event_listener,
                self.event_loop.make_dispatcher(self.handle_debugger_event))
        self.listener_handler_token = listener_handler.start()
        return { 'supportsConfigurationDoneRequest': True,
                 'supportsEvaluateForHovers': True,
                 'supportsFunctionBreakpoints': True,
                 'supportsConditionalBreakpoints': True,
                 'supportsSetVariable': True }

    def DEBUG_launch(self, args):
        self.exec_commands(args.get('initCommands'))
        self.target = self.create_target(args)
        self.send_event('initialized', {})
        # defer actual launching till configurationDone request, so that
        # we can receive and set initial breakpoints before the target starts running
        self.do_launch = self.launch
        self.launch_args = args
        return AsyncResponse

    def launch(self, args):
        log.info('Launching...')
        self.exec_commands(args.get('preRunCommands'))
        flags = 0
        # argumetns
        target_args = args.get('args', None)
        if target_args is not None:
            if isinstance(target_args, string_type):
                target_args = shlex.split(target_args)
            target_args = [str(arg) for arg in target_args]
        # environment
        env = args.get('env', None)
        envp = [str('%s=%s' % pair) for pair in os.environ.items()]
        if env is not None: # Convert dict to a list of 'key=value' strings
            envp = envp + ([str('%s=%s' % pair) for pair in env.items()])
        # stdio
        stdio, extra_flags = self.configure_stdio(args)
        flags |= extra_flags
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
        self.process_launched = True

    def configure_stdio(self, args):
        stdio = args.get('stdio', None)
        missing = () # None is a valid value here, so we need a new one to designate 'missing'
        if isinstance(stdio, dict): # Flatten it into a list
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
        # Map '*' to None
        stdio = [None if s == '*' else s for s in stdio]
        # open a new terminal window if needed
        extra_flags = 0
        if None in stdio:
            term_type = args.get('terminal', 'console')
            if term_type == 'external':
                if 'linux' in sys.platform:
                    self.terminal = terminal.create()
                    term_fd = self.terminal.tty
                else:
                    # OSX LLDB supports this natively.
                    # On Windows LLDB always creates a new console window (even if stdio is redirected).
                    extra_flags = lldb.eLaunchFlagLaunchInTTY | lldb.eLaunchFlagCloseTTYOnExit
                    term_fd = None
            elif term_type == 'integrated':
                self.terminal = terminal.create(self.spawn_vscode_terminal)
                term_fd = self.terminal.tty
            else:
                term_fd = None # that'll send them to VSCode debug console
            stdio = [term_fd if s is None else str(s) for s in stdio]
        return stdio, extra_flags

    def spawn_vscode_terminal(self, command):
        on_complete = lambda ok, body: None
        self.send_request('runInTerminal', {
            'kind': 'external', 'cwd': None,
            'args': ['bash', '-c', command] }, on_complete)

    def DEBUG_attach(self, args):
        self.exec_commands(args.get('initCommands'))
        self.target = self.create_target(args)
        self.send_event('initialized', {})
        self.do_launch = self.attach
        self.launch_args = args
        return AsyncResponse

    def attach(self, args):
        log.info('Attaching...')
        self.exec_commands(args.get('preRunCommands'))

        error = lldb.SBError()
        if args.get('pid', None) is not None:
            self.process = self.target.AttachToProcessWithID(self.event_listener, args['pid'], error)
        else:
            self.process = self.target.AttachToProcessWithName(self.event_listener, str(args['program']), False, error)
        if not error.Success():
            self.console_msg(error.GetCString())
            raise UserError('Failed to attach to process.')
        assert self.process.IsValid()
        self.process_launched = False
        if not args.get('stopOnEntry', False):
            self.process.Continue()

    def create_target(self, args):
        program = args['program']
        load_dependents = not args.get('noDebug', False)
        error = lldb.SBError()
        target = self.debugger.CreateTarget(str(program), lldb.LLDB_ARCH_DEFAULT, None, load_dependents, error)
        if not error.Success() and 'win32' in sys.platform:
            # On Windows, try appending '.exe' extension, to make launch configs more uniform.
            program += '.exe'
            error2 = lldb.SBError()
            target = self.debugger.CreateTarget(str(program), lldb.LLDB_ARCH_DEFAULT, None, load_dependents, error2)
            if error2.Success():
                args['program'] = program
        if not error.Success():
            raise UserError('Could not initialize debug target: ' + error.GetCString())
        target.GetBroadcaster().AddListener(self.event_listener, lldb.SBTarget.eBroadcastBitBreakpointChanged)
        return target

    def exec_commands(self, commands):
        if commands is not None:
            interp = self.debugger.GetCommandInterpreter()
            result = lldb.SBCommandReturnObject()
            for command in commands:
                interp.HandleCommand(str(command), result)
                output = result.GetOutput() if result.Succeeded() else result.GetError()
                self.console_msg(output)

    def DEBUG_setBreakpoints(self, args):
        if self.launch_args.get('noDebug', False):
            return

        result = []
        self.ignore_bp_events = True
        source = args['source']

        in_dasm = True
        file_id = source.get('sourceReference')
        if file_id is None:
            file_id = opt_str(source.get('path'))
            in_dasm = False

        if file_id is not None:
            req_bps = args['breakpoints']
            req_bp_lines = [req['line'] for req in req_bps]
            # Existing breakpints indexed by line
            curr_bps = self.breakpoints.setdefault(file_id, {})
            # Existing breakpints that were removed
            for line, bp in list(curr_bps.items()):
                if line not in req_bp_lines:
                    self.target.BreakpointDelete(bp.GetID())
                    del curr_bps[line]
            # Added or updated
            for req in req_bps:
                line = req['line']
                bp = curr_bps.get(line, None)
                if bp is None:
                    if not in_dasm:
                        bp = self.target.BreakpointCreateByLocation(file_id, line)
                    else:
                        dasm = self.disassembly_by_handle.get(file_id)
                        addr = dasm.address_by_line_num(line)
                        bp = self.target.BreakpointCreateByAddress(addr)
                        bp.dont_resolve = True
                    self.set_bp_condition(bp, req)
                    curr_bps[line] = bp
                result.append(self.make_bp_resp(bp))
        else:
            result.append({'verified': False})

        self.ignore_bp_events = False
        return { 'breakpoints': result }

    def DEBUG_setFunctionBreakpoints(self, args):
        if self.launch_args.get('noDebug', False):
            return

        result = []
        self.ignore_bp_events = True
        # Breakpoint requests indexed by function name
        req_bps = args['breakpoints']
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
                if name.startswith('/'):
                    bp = self.target.BreakpointCreateByRegex(str(name[1:]))
                else:
                    bp = self.target.BreakpointCreateByName(str(name))
                self.set_bp_condition(bp, req)
                self.fn_breakpoints[name] = bp
            result.append(self.make_bp_resp(bp))
        self.ignore_bp_events = False

        return { 'breakpoints': result }

    def set_bp_condition(self, bp, req):
        cond = opt_str(req.get('condition', None))
        if cond != bp.GetCondition():
            bp.SetCondition(cond)

    # Create breakpoint location info for a response message
    def make_bp_resp(self, bp):
        if bp.num_locations == 0:
            return { 'id': bp.GetID(), 'verified': False }
        if getattr(bp, 'dont_resolve', False): # these originate from disassembly
             return { 'id': bp.GetID(), 'verified': True }
        le = bp.GetLocationAtIndex(0).GetAddress().GetLineEntry()
        fs = le.GetFileSpec()
        if not (le.IsValid() and fs.IsValid()):
            return { 'id': bp.GetID(), 'verified': True }
        source = { 'name': fs.basename, 'path': fs.fullpath }
        return { 'id': bp.GetID(), 'verified': True, 'source': source, 'line': le.line }

    def DEBUG_setExceptionBreakpoints(self, args):
        if not self.launch_args.get('noDebug', False):
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
        'cpp': (lambda target: target.BreakpointCreateForException(lldb.eLanguageTypeC_plus_plus, False, True),
                lambda target: target.BreakpointCreateByName('terminate')),
    }

    def DEBUG_configurationDone(self, args):
        try:
            result = self.do_launch(self.launch_args)
        except Exception as e:
            result = e
        # do_launch is asynchronous so we need to send its result
        self.send_response(self.launch_args['response'], result)
        # LLDB doesn't seem to automatically generate a stop event for stop_on_entry
        if self.process is not None and self.process.GetState() == lldb.eStateStopped:
            self.notify_target_stopped(None)

    def DEBUG_pause(self, args):
        self.process.Stop()

    def DEBUG_continue(self, args):
        # variable handles will be invalid after running,
        # so we may as well clean them up now
        self.var_refs.reset()
        self.process.Continue()

    def DEBUG_next(self, args):
        self.var_refs.reset()
        tid = args['threadId']
        thread = self.process.GetThreadByID(tid)
        if not self.in_disassembly(thread.GetFrameAtIndex(0)):
            thread.StepOver()
        else:
            thread.StepInstruction(True)

    def DEBUG_stepIn(self, args):
        self.var_refs.reset()
        tid = args['threadId']
        thread = self.process.GetThreadByID(tid)
        if not self.in_disassembly(thread.GetFrameAtIndex(0)):
            thread.StepInto()
        else:
            thread.StepInstruction(False)

    def DEBUG_stepOut(self, args):
        self.var_refs.reset()
        tid = args['threadId']
        thread = self.process.GetThreadByID(tid)
        thread.StepOut()

    def DEBUG_threads(self, args):
        threads = []
        for thread in self.process:
            tid = thread.GetThreadID()
            threads.append({ 'id': tid, 'name': '%s:%d' % (thread.GetName(), tid) })
        return { 'threads': threads }

    def DEBUG_stackTrace(self, args):
        thread = self.process.GetThreadByID(args['threadId'])
        start_frame = args.get('startFrame', 0)
        levels = args.get('levels', sys.maxsize)
        if start_frame + levels > thread.num_frames:
            levels = thread.num_frames - start_frame
        stack_frames = []
        for i in range(start_frame, start_frame + levels):
            frame = thread.frames[i]
            stack_frame = { 'id': self.var_refs.create(frame, frame.GetFP(), None) }
            fn_name = frame.GetFunctionName()
            if fn_name is None:
                fn_name = str(frame.GetPCAddress())
            stack_frame['name'] = fn_name

            if not self.in_disassembly(frame):
                le = frame.GetLineEntry()
                if le.IsValid():
                    fs = le.GetFileSpec()
                    # VSCode gets confused if the path contains funky stuff like a double-slash
                    full_path = os.path.normpath(fs.fullpath)
                    stack_frame['source'] = { 'name': fs.basename, 'path': full_path }
                    stack_frame['line'] = le.GetLine()
                    stack_frame['column'] = le.GetColumn()
            else:
                pc_addr = frame.GetPCAddress().GetLoadAddress(self.target)
                dasm = disassembly.find(self.disassembly_by_addr, pc_addr)
                if dasm is None:
                    log.info('Creating new disassembly for %x', pc_addr)
                    dasm = disassembly.Disassembly(frame, self.target)
                    disassembly.insert(self.disassembly_by_addr, dasm)
                    dasm.source_ref = self.disassembly_by_handle.create(dasm)
                stack_frame['source'] = dasm.get_source_ref()
                stack_frame['line'] = dasm.line_num_by_address(pc_addr)
                stack_frame['column'] = 0

            stack_frames.append(stack_frame)
        return { 'stackFrames': stack_frames, 'totalFrames': len(thread) }

    def in_disassembly(self, frame):
        le = frame.GetLineEntry()
        if self.show_disassembly == 'never':
            return False
        elif self.show_disassembly == 'always':
            return True
        else:
            return not le.IsValid()

    def DEBUG_source(self, args):
        sourceRef = int(args['sourceReference'])
        dasm = self.disassembly_by_handle.get(sourceRef)
        return { 'content': dasm.get_source_text(), 'mimeType': 'text/x-lldb.disassembly' }

    def DEBUG_scopes(self, args):
        frame_id = args['frameId']
        frame = self.var_refs.get(frame_id)
        locals = { 'name': 'Local', 'variablesReference': frame_id, 'expensive': False }
        statics_scope_handle = self.var_refs.create(StaticsScope(frame), '[stat]', frame_id)
        statics = { 'name': 'Static', 'variablesReference': statics_scope_handle, 'expensive': False }
        regs_scope_handle = self.var_refs.create(RegistersScope(frame), '[regs]', frame_id)
        registers = { 'name': 'CPU Registers', 'variablesReference': regs_scope_handle, 'expensive': False }
        return { 'scopes': [locals, statics, registers] }

    def DEBUG_variables(self, args):
        container_handle = args['variablesReference']
        container = self.var_refs.get(container_handle)
        if container is None:
            raise Exception('Invalid variables reference')
        if isinstance(container, lldb.SBFrame):
            # args, locals, statics, in_scope_only
            vars = container.GetVariables(True, True, False, True)
        elif isinstance(container, StaticsScope):
            vars = container.frame.GetVariables(False, False, True, True)
        elif isinstance(container, RegistersScope):
            vars = container.frame.GetRegisters()
        elif isinstance(container, lldb.SBValue):
            vars = container

        variables = []
        for var in vars:
            name, value, dtype, handle = self.parse_var(var, self.global_format, container_handle)
            # Sometimes LLDB returns junk entries with empty names and values
            if name is not None:
                variable = { 'name': name, 'value': value, 'type': dtype, 'variablesReference': handle }
                variables.append(variable)

        # If this node was synthetic (i.e. a product of a visualizer),
        # append a [raw] child, which can be expended to snow raw data.
        if isinstance(vars, lldb.SBValue) and vars.IsSynthetic():
            handle = self.var_refs.create(vars.GetNonSyntheticValue(), '[raw]', container_handle)
            variable = { 'name': '[raw]', 'value': vars.GetTypeName(), 'variablesReference': handle }
            variables.append(variable)

        return { 'variables': variables }

    def DEBUG_evaluate(self, args):
        context = args['context']
        expr = str(args['expression'])
        if context in ['watch', 'hover']:
            return self.evaluate_expr(args, expr)
        elif expr.startswith('?'): # "?<expr>" in 'repl' context
            return self.evaluate_expr(args, expr[1:])
        # Else evaluate as debugger command

        # set up evaluation context
        frame = self.var_refs.get(args.get('frameId'), None)
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
        frame = self.var_refs.get(args.get('frameId'), None)
        if frame is None:
            raise Exception('Missing frameId')

        format = self.global_format
        for suffix, fmt in self.format_codes:
            if expr.endswith(suffix):
                format = fmt
                expr = expr[:-len(suffix)]
                break

        var = frame.EvaluateExpression(expr)
        error = var.GetError()
        if error.Success():
            _, value, dtype, handle = self.parse_var(var, format)
            return { 'result': value, 'type': dtype, 'variablesReference': handle }
        else:
            message = error.GetCString()
            if args['context'] == 'repl':
                self.console_msg(message)
                return None
            else:
                raise UserError(message.replace('\n', '; '), no_console=True)

    format_codes = [(',h', lldb.eFormatHex),
                    (',x', lldb.eFormatHex),
                    (',o', lldb.eFormatOctal),
                    (',d', lldb.eFormatDecimal),
                    (',b', lldb.eFormatBinary),
                    (',f', lldb.eFormatFloat),
                    (',p', lldb.eFormatPointer),
                    (',u', lldb.eFormatUnsigned),
                    (',s', lldb.eFormatCString),
                    (',y', lldb.eFormatBytes),
                    (',Y', lldb.eFormatBytesWithASCII)]

    def parse_var(self, var, format, parent_handle=None):
        name = var.GetName()
        value = self.get_var_value(var, format)
        dtype = var.GetTypeName()
        if var.GetNumChildren() > 0:
            handle = self.var_refs.create(var, name, parent_handle)
            if value is None:
                value = dtype
            if value is None:
                value = ''
        else:
            handle = 0
            if value is None:
                value = '<not available>'
        return name, value, dtype, handle

    def get_var_value(self, var, format):
        var.SetFormat(format)
        value = var.GetValue()
        if value is None:
            value = var.GetSummary()
            if value is not None:
                value = value.replace('\n', '') # VSCode won't display line breaks
        if PY2 and value is not None:
            value = value.decode('latin1') # or else json will try to treat it as utf8
        return value

    def DEBUG_setVariable(self, args):
        container = self.var_refs.get(args['variablesReference'])
        if container is None:
            raise Exception('Invalid variables reference')

        name = str(args['name'])
        if isinstance(container, lldb.SBFrame):
            # args, locals, statics, in_scope_only
            var = container.FindVariable(name)
        elif isinstance(container, lldb.SBValue):
            var = container.GetChildMemberWithName(name)
            if not var.IsValid():
                var = container.GetValueForExpressionPath(name)
        if not var.IsValid():
            raise Exception('Could not get a child with name ' + name)

        error = lldb.SBError()
        if not var.SetValueFromCString(str(args['value']), error):
            raise UserError(error.GetCString())
        return { 'value': self.get_var_value(var, self.global_format) }

    def DEBUG_disconnect(self, args):
        if self.process:
            if self.process_launched:
                self.process.Kill()
            else:
                self.process.Detach()
        self.process = None
        self.target = None
        self.terminal = None
        self.event_loop.stop()

    def EXTENSION_test(self, args):
        self.console_msg('TEST\n')

    def EXTENSION_showDisassembly(self, args):
        value = args.get('value', 'toggle')
        if value == 'toggle':
            self.show_disassembly = 'auto' if self.show_disassembly != 'auto' else 'always'
        else:
            self.show_disassembly = value
        self.refresh_client_display()

    def EXTENSION_displayFormat(self, args):
        value = args.get('value', 'auto')
        if value == 'hex':
            self.global_format = lldb.eFormatHex
        elif value == 'decimal':
            self.global_format = lldb.eFormatDecimal
        elif value == 'binary':
            self.global_format = lldb.eFormatBinary
        else:
            self.global_format = lldb.eFormatDefault
        self.refresh_client_display()

    # Fake a target stop to force VSCode to refresh the display
    def refresh_client_display(self):
        thread_id = self.process.GetSelectedThread().GetThreadID()
        self.send_event('stopped', { 'reason': 'mode switch',
                                     'threadId': thread_id,
                                     'allThreadsStopped': True })

    # handles messages from VSCode debug client
    def handle_message(self, message):
        if message is None:
            # Client connection lost; treat this the same as a normal disconnect.
            self.disconnect_request(None)
            return

        if message['type'] == 'response':
            seq = message['request_seq']
            on_complete = self.pending_requests.get(seq)
            if on_complete is None:
                log.error('Received response without pending request, seq=%d', seq)
                return
            del self.pending_requests[seq]
            if message['success']:
                on_complete(True, message.get('body'))
            else:
                on_complete(False, message.get('message'))
        else: # request
            command =  message['command']
            args = message.get('arguments', {})
            # Prepare response - in case the handler is async
            response = { 'type': 'response', 'command': command,
                         'request_seq': message['seq'], 'success': False }
            args['response'] = response

            log.debug('### Handling command: %s', command)
            handler = getattr(self, 'DEBUG_' + command, None)
            if handler is not None:
                try:
                    result = handler(args)
                    # `result` being an AsyncResponse means that the handler is asynchronous and
                    # will respond at a later time.
                    if result is AsyncResponse: return
                except Exception as e:
                    result = e
                self.send_response(response, result)
            else:
                log.warning('No handler for %s', command)
                response['success'] = False
                self.send_message(response)

    # sends response with `result` as a body
    def send_response(self, response, result):
        if result is None or isinstance(result, dict):
            response['success'] = True
            response['body'] = result
        elif isinstance(result, UserError):
            if not result.no_console:
                self.console_msg('Error: ' + str(result))
            response['success'] = False
            response['body'] = { 'error': { 'id': 0, 'format': str(result), 'showUser': True } }
        elif isinstance(result, Exception):
            tb = traceback.format_exc(result)
            log.error('Internal debugger error:\n' + tb)
            self.console_msg('Internal debugger error:\n' + tb)
            msg = 'Internal debugger error: ' + str(result)
            response['success'] = False
            response['body'] = { 'error': { 'id': 0, 'format': msg, 'showUser': True } }
        else:
            assert False, "Invalid result type: %s" % result
        self.send_message(response)

    # send a request to VSCode. When response is received, on_complete(True, request.body)
    # will be called on success, or on_complete(False, request.message) on failure.
    def send_request(self, command, args, on_complete):
        request = { 'type': 'request', 'seq': self.request_seq, 'command': command,
                    'arguments': args }
        self.pending_requests[self.request_seq] = on_complete
        self.request_seq += 1
        self.send_message(request)

    # Handle messages from VSCode extension
    def handle_extension_message(self, request):
        if request is None:
            return # the client has disconnected
        command =  request['command']
        args = request.get('arguments', {})
        response = { 'type': 'response', 'command': command,
                     'request_seq': request['seq'], 'success': False }
        args['response'] = response

        log.debug('### Handling extension command: %s', command)
        handler = getattr(self, 'EXTENSION_' + command, None)
        if handler is not None:
            try:
                result = handler(args)
                response['success'] = True
                response['body'] = result
                self.send_extension_message(response)
            except Exception as e:
                tb = traceback.format_exc(e)
                log.error('Internal debugger error:\n' + tb)
                msg = str(e)
                response['success'] = False
                response['body'] = { 'error': { 'id': 0, 'format': msg, 'showUser': True } }
                self.send_extension_message(response)
        else:
            log.warning('No handler for extension command %s', command)
            response['success'] = False
            self.send_extension_message(response)

    # Handles debugger notifications
    def handle_debugger_event(self, event):
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
        elif lldb.SBBreakpoint.EventIsBreakpointEvent(event) and not self.ignore_bp_events:
            self.notify_breakpoint(event)

    def notify_target_stopped(self, lldb_event):
        event = { 'allThreadsStopped': True } # LLDB always stops all threads
        # Find the thread that has caused this stop
        thread_id = None
        stopped_thread = None
        stop_reason = 'unknown'
        for thread in self.process:
            stop_reason = thread.GetStopReason()
            if stop_reason == lldb.eStopReasonBreakpoint:
                stopped_thread = thread
                event['threadId'] = thread.GetThreadID()
                bp_id = thread.GetStopReasonDataAtIndex(0)
                for bp in self.exc_breakpoints:
                    if bp.GetID() == bp_id:
                        stop_reason = 'exception'
                        break;
                else:
                    stop_reason = 'breakpoint'
                break
            elif stop_reason == lldb.eStopReasonException:
                stopped_thread = thread
                stop_reason = 'exception'
                break
            elif stop_reason in [lldb.eStopReasonTrace, lldb.eStopReasonPlanComplete]:
                stopped_thread = thread
                stop_reason = 'step'
                break
            elif stop_reason == lldb.eStopReasonSignal:
                stopped_thread = thread
                stop_reason = 'signal'
                event['text'] = thread.GetStopReasonDataAtIndex(0)
                break
        event['reason'] = stop_reason
        if thread is not None:
            self.process.SetSelectedThread(stopped_thread)
            event['threadId'] = stopped_thread.GetThreadID()
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
            self.send_event('output', { 'category': category, 'output': output })
            output = read_stream(1024)

    def notify_breakpoint(self, event):
        return
        bp = lldb.SBBreakpoint.GetBreakpointFromEvent(event)
        bp_info = self.make_bp_resp(bp)
        self.send_event('breakpoint', { 'reason': 'new', 'breakpoint': bp_info })

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
    def __init__(self, message, no_console=False):
        Exception.__init__(self, message)
        # Don't copy error message to debug console if this is set
        self.no_console = no_console

# Result type for async handlers
class AsyncResponse:
    pass

class StaticsScope:
    def __init__(self, frame):
        self.frame = frame

class RegistersScope:
    def __init__(self, frame):
        self.frame = frame

def opt_str(s):
    return str(s) if s != None else None

string_type = basestring if PY2 else str
