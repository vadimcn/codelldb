import sys
import logging
import os.path
import shlex
import traceback
import collections
import tempfile
import lldb
from . import expressions
from . import debugevents
from . import disassembly
from . import handles
from . import terminal
from . import formatters
from . import PY2, is_string, from_lldb_str, to_lldb_str, xrange

log = logging.getLogger('debugsession')
log.info('Imported')

# The maximum number of children we'll retrieve for a container value.
# This is to cope with the not yet initialized objects whose length fields contain garbage.
MAX_VAR_CHILDREN = 10000

class DebugSession:

    def __init__(self, parameters, event_loop, send_message):
        DebugSession.current = self
        self.parameters = parameters
        self.event_loop = event_loop
        self.send_message = send_message
        self.var_refs = handles.HandleTree()
        self.ignore_bp_events = False
        self.line_breakpoints = dict() # { file_id : { line : breakpoint_id } }
        self.addr_breakpoints = set() # a list breakpoints that were set by address
        self.fn_breakpoints = dict() # { fn_name : breakpoint_id }
        self.exc_breakpoints = set() # list of exception breakpoints
        self.breakpoint_conditions = dict() # { bp_id : code_object }
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
        self.known_threads = set()

    def DEBUG_initialize(self, args):
        self.line_offset = 0 if args.get('linesStartAt1', True) else 1
        self.col_offset = 0 if args.get('columnsStartAt1', True) else 1

        self.debugger = lldb.debugger if lldb.debugger else lldb.SBDebugger.Create()
        log.info('LLDB version: %s', self.debugger.GetVersionString())
        self.debugger.SetAsync(True)

        self.debugger.HandleCommand('script import adapter, debugger')

        # The default event handler spams debug console each time we hit a brakpoint.
        # Tell debugger's event listener to ignore process state change events.
        default_listener = self.debugger.GetListener()
        default_listener.StopListeningForEventClass(self.debugger,
            lldb.SBProcess.GetBroadcasterClassName(), lldb.SBProcess.eBroadcastBitStateChanged)

        # Create our event listener and spawn a worker thread to poll it.
        self.event_listener = lldb.SBListener('DebugSession')
        listener_handler = debugevents.AsyncListener(self.event_listener,
                self.event_loop.make_dispatcher(self.handle_debugger_event))
        self.listener_handler_token = listener_handler.start()

        # Hook up debugger's stdout and stderr so we can redirect them to VSCode console
        r, w = os.pipe()
        read_end = os.fdopen(r, 'r')
        write_end = os.fdopen(w, 'w', 1) # line-buffered
        debugger_output_listener = debugevents.DebuggerOutputListener(read_end,
                self.event_loop.make_dispatcher(self.handle_debugger_output))
        self.debugger_output_listener_token = debugger_output_listener.start()
        self.debugger.SetOutputFileHandle(write_end, False)
        self.debugger.SetErrorFileHandle(write_end, False)
        sys.stdout = write_end
        sys.stderr = write_end
        return {
            'supportsConfigurationDoneRequest': True,
            'supportsEvaluateForHovers': True,
            'supportsFunctionBreakpoints': True,
            'supportsConditionalBreakpoints': True,
            'supportsHitConditionalBreakpoints': True,
            'supportsSetVariable': True,
            'supportsCompletionsRequest': True,
            'supportTerminateDebuggee': True,
            'supportsDelayedStackTraceLoading': True,
            'supportsStepBack': self.parameters.get('reverseDebugging', False)
        }

    def DEBUG_launch(self, args):
        if args.get('request') == 'custom':
            return self.custom_launch(args)
        self.exec_commands(args.get('initCommands'))
        self.target = self.create_target(args)
        self.send_event('initialized', {})
        # defer actual launching till configurationDone request, so that
        # we can receive and set initial breakpoints before the target starts running
        self.do_launch = self.complete_launch
        self.launch_args = args
        return AsyncResponse

    def complete_launch(self, args):
        log.info('Launching...')
        self.exec_commands(args.get('preRunCommands'))
        flags = 0
        # argumetns
        target_args = args.get('args', None)
        if target_args is not None:
            if is_string(target_args):
                target_args = shlex.split(target_args)
            target_args = [to_lldb_str(arg) for arg in target_args]
        # environment
        env = args.get('env', None)
        envp = [to_lldb_str('%s=%s' % pair) for pair in os.environ.items()]
        if env is not None: # Convert dict to a list of 'key=value' strings
            envp = envp + ([to_lldb_str('%s=%s' % pair) for pair in env.items()])
        # stdio
        stdio, extra_flags = self.configure_stdio(args)
        flags |= extra_flags
        # working directory
        work_dir = opt_lldb_str(args.get('cwd', None))
        stop_on_entry = args.get('stopOnEntry', False)
        # launch!
        error = lldb.SBError()
        log.info('%s %s %s', target_args, envp, work_dir)
        self.process = self.target.Launch(self.event_listener,
            target_args, envp, stdio[0], stdio[1], stdio[2],
            work_dir, flags, stop_on_entry, error)
        if not error.Success():
            self.console_msg(error.GetCString())
            self.send_event('terminated', {})
            raise UserError('Process launch failed.')

        assert self.process.IsValid()
        self.process_launched = True

    def DEBUG_attach(self, args):
        pid = args.get('pid', None)
        program = args.get('program', None)
        if pid is None and program is None:
            raise UserError('Either the \'program\' or the \'pid\' must be specified.')
        self.exec_commands(args.get('initCommands'))
        self.target = self.create_target(args)
        self.send_event('initialized', {})
        self.do_launch = self.complete_attach
        self.launch_args = args
        return AsyncResponse

    def complete_attach(self, args):
        log.info('Attaching...')
        self.exec_commands(args.get('preRunCommands'))
        error = lldb.SBError()
        pid = args.get('pid', None)
        if pid is not None:
            if is_string(pid): pid = int(pid)
            self.process = self.target.AttachToProcessWithID(self.event_listener, pid, error)
        else:
            program = to_lldb_str(args['program'])
            self.process = self.target.AttachToProcessWithName(self.event_listener, program, False, error)
        if not error.Success():
            self.console_msg(error.GetCString())
            raise UserError('Failed to attach to the process.')
        assert self.process.IsValid()
        self.process_launched = False
        if not args.get('stopOnEntry', False):
            self.process.Continue()

    def custom_launch(self, args):
        self.exec_commands(args.get('initCommands'))
        self.target = self.debugger.GetSelectedTarget()
        if not self.target.IsValid():
            self.console_msg('Warning: target is invalid after running "initCommands"')
        self.target.GetBroadcaster().AddListener(self.event_listener, lldb.SBTarget.eBroadcastBitBreakpointChanged)
        self.send_event('initialized', {})
        self.do_launch = self.complete_custom_launch
        self.launch_args = args
        return AsyncResponse

    def complete_custom_launch(self, args):
        log.info('Custom launching...')
        self.exec_commands(args.get('preRunCommands'))
        self.process = self.target.GetProcess()
        if not self.process.IsValid():
            self.console_msg('Warning: process is invalid after running "preRunCommands"')
        self.process.GetBroadcaster().AddListener(self.event_listener, 0xFFFFFF)
        self.process_launched = False

    def create_target(self, args):
        program = args.get('program')
        if program is not None:
            load_dependents = not args.get('noDebug', False)
            error = lldb.SBError()
            target = self.debugger.CreateTarget(to_lldb_str(program), None, None, load_dependents, error)
            if not error.Success() and 'win32' in sys.platform:
                # On Windows, try appending '.exe' extension, to make launch configs more uniform.
                program += '.exe'
                error2 = lldb.SBError()
                target = self.debugger.CreateTarget(to_lldb_str(program), None, None, load_dependents, error2)
                if error2.Success():
                    args['program'] = program
                    error.Clear()
            if not error.Success():
                raise UserError('Could not initialize debug target: ' + error.GetCString())
        else:
            if args['request'] == 'launch':
                raise UserError('Program path is required for launch.')
            target = self.debugger.CreateTarget('') # OK if attaching by pid
        target.GetBroadcaster().AddListener(self.event_listener, lldb.SBTarget.eBroadcastBitBreakpointChanged)
        return target

    def pre_launch(self):
        expressions.init_formatters(self.debugger)

    def exec_commands(self, commands):
        if commands is not None:
            interp = self.debugger.GetCommandInterpreter()
            result = lldb.SBCommandReturnObject()
            for command in commands:
                interp.HandleCommand(to_lldb_str(command), result)
                sys.stdout.flush()
                output = result.GetOutput() if result.Succeeded() else result.GetError()
                self.console_msg(output)
            sys.stdout.flush()

    def configure_stdio(self, args):
        stdio = args.get('stdio', None)
        missing = () # None is a valid value here, so we need a new one to designate 'missing'
        if isinstance(stdio, dict): # Flatten it into a list
            stdio = [stdio.get('stdin', missing),
                     stdio.get('stdout', missing),
                     stdio.get('stderr', missing)]
        elif stdio is None or is_string(stdio):
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
            if 'win32' not in sys.platform:
                if term_type in ['integrated', 'external']:
                    self.terminal = terminal.create(
                        lambda args: self.spawn_vscode_terminal(kind=term_type, args=args))
                    term_fd = self.terminal.tty
                else:
                    term_fd = None # that'll send them to VSCode debug console
            else: # Windows
                no_console = 'false' if term_type == 'external' else 'true'
                os.environ['LLDB_LAUNCH_INFERIORS_WITHOUT_CONSOLE'] = no_console
                term_fd = None # no other options on Windows
            stdio = [term_fd if s is None else to_lldb_str(s) for s in stdio]
        return stdio, extra_flags

    def spawn_vscode_terminal(self, kind, args=[], cwd='', env={}, title='Debuggee'):
        on_complete = lambda ok, body: None
        self.send_request('runInTerminal', {
                'kind': kind, 'cwd': cwd, 'args': args, 'env': env, 'title': title
            }, on_complete)

    def DEBUG_setBreakpoints(self, args):
        if self.launch_args.get('noDebug', False):
            return

        result = []
        self.ignore_bp_events = True
        source = args['source']

        # Disassembly breakpoints have only sourceReference, normal breakpoints have a file path.
        in_dasm = True
        file_id = source.get('sourceReference')
        if file_id is None:
            file_id = opt_lldb_str(source.get('path'))
            in_dasm = False

        if file_id is not None:
            req_bps = args['breakpoints']
            req_bp_lines = [req['line'] for req in req_bps]
            # Existing breakpints indexed by line
            file_bps = self.line_breakpoints.setdefault(file_id, {})
            # Existing breakpints that were removed
            for line, bp_id in list(file_bps.items()):
                if line not in req_bp_lines:
                    self.target.BreakpointDelete(bp_id)
                    del file_bps[line]
                    self.breakpoint_conditions.pop(bp_id, None)
                    self.addr_breakpoints.discard(bp_id)
            # Added or updated
            for req in req_bps:
                line = req['line']
                bp_id = file_bps.get(line, None)
                if bp_id is None:
                    if not in_dasm:
                        # LLDB is pretty finicky about breakpoint location path exactly matching
                        # the source path found in debug info.  Unfortunately, this means that
                        # '/some/dir/file.c' and '/some/dir/./file.c' are not considered the same
                        # file, and debug info contains un-normalized paths like this pretty often.
                        # The workaroud is to set a breakpoint by file name and line only, then
                        # check all resolved locations and filter out the ones that don't match
                        # the full path.
                        file_name = os.path.basename(file_id)
                        bp = self.target.BreakpointCreateByLocation(file_name, line)
                        for loc in bp:
                            fs = loc.GetAddress().GetLineEntry().GetFileSpec()
                            if fs.IsValid():
                                bp_path = self.map_path_to_local(fs.fullpath)
                                if not same_path(bp_path, file_id):
                                    loc.SetEnabled(False)
                    else:
                        dasm = self.disassembly_by_handle.get(file_id)
                        addr = dasm.address_by_line_num(line)
                        bp = self.target.BreakpointCreateByAddress(addr)
                        self.addr_breakpoints.add(bp.GetID())
                    self.set_bp_condition(bp, req)
                    file_bps[line] = bp.GetID()
                else:
                    bp = self.target.FindBreakpointByID(bp_id)
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
        for name, bp_id in list(self.fn_breakpoints.items()):
            if name not in req_bp_names:
                self.target.BreakpointDelete(bp_id)
                del self.fn_breakpoints[name]
                self.breakpoint_conditions.pop(bp_id, None)
        # Added or updated
        result = []
        for req in req_bps:
            name = req['name']
            bp_id = self.fn_breakpoints.get(name, None)
            if bp_id is None:
                if name.startswith('/re '):
                    bp = self.target.BreakpointCreateByRegex(to_lldb_str(name[4:]))
                else:
                    bp = self.target.BreakpointCreateByName(to_lldb_str(name))
                self.set_bp_condition(bp, req)
                self.fn_breakpoints[name] = bp.GetID()
            else:
                bp = self.target.FindBreakpointByID(bp_id)
            result.append(self.make_bp_resp(bp))
        self.ignore_bp_events = False

        return { 'breakpoints': result }

    # Sets up breakpoint stopping condition
    def set_bp_condition(self, bp, req):
        cond = opt_lldb_str(req.get('condition', None))
        if cond is not None:
            if cond.startswith('/nat '):
                # LLDB native expression
                bp.SetCondition(cond[5:])
            else:
                if cond.startswith('/py '):
                    # Python expression
                    pp_cond = expressions.preprocess_varsubsts(cond[4:])
                    try:
                        pycode = compile(pp_cond, '<string>', 'eval')
                    except Exception as e:
                        self.console_msg('Could not set breakpoint condition "%s": %s' % (cond, str(e)))
                        return

                    def eval_condition(frame, eval_globals):
                        self.set_selected_frame(frame)
                        eval_globals['__frame_vars'] = expressions.PyEvalContext(frame)
                        return eval(pycode, eval_globals, {})
                else:
                    # Simple expression
                    pp_cond = expressions.preprocess(cond)
                    try:
                        pycode = compile(pp_cond, '<string>', 'eval')
                    except Exception as e:
                        self.console_msg('Could not set breakpoint condition "%s": %s' % (cond, str(e)))
                        return

                    def eval_condition(frame, eval_globals):
                        return eval(pycode, self.pyeval_globals, expressions.PyEvalContext(frame))

                self.breakpoint_conditions[bp.GetID()] = eval_condition
                bp.SetScriptCallbackFunction('adapter.debugsession.on_breakpoint_hit')

        ignoreCount = req.get('hitCondition', None)
        if ignoreCount is not None:
            try:
                bp.SetIgnoreCount(int(ignoreCount))
            except ValueError:
                self.console_msg('Could not parse hit count: %s' % ignoreCount)

    # Create breakpoint location info for a response message.
    def make_bp_resp(self, bp):
        if bp.GetID() in self.addr_breakpoints: # these originate from disassembly
            return { 'id': bp.GetID(), 'verified': True }
        bp_resp =  { 'id': bp.GetID() }
        for bp_loc in bp:
            if bp_loc.IsEnabled():
                le = bp_loc.GetAddress().GetLineEntry()
                if le.IsValid():
                    fs = le.GetFileSpec()
                    path = self.map_path_to_local(fs.fullpath)
                    bp_resp['source'] = { 'name': fs.basename, 'path': path }
                    bp_resp['line'] = le.line
                    bp_resp['verified'] = True
                    break
        return bp_resp

    def should_stop_on_bp(self, bp_id, frame, internal_dict):
        cond = self.breakpoint_conditions.get(bp_id)
        if cond is None:
            return True
        try:
            return cond(frame, internal_dict)
        except Exception as e:
            self.console_msg('Could not evaluate breakpoint condition: %s' % traceback.format_exc())
            return True

    def DEBUG_setExceptionBreakpoints(self, args):
        if not self.launch_args.get('noDebug', False):
            filters = args['filters']
            for bp_id in self.exc_breakpoints:
                self.target.BreakpointDelete(bp_id)
            self.exc_breakpoints.clear()

            set_all = 'all' in filters
            set_uncaught = 'uncaught' in filters
            for lang in self.launch_args.get('sourceLanguages', []):
                language = languages.get(lang.lower())
                if language is not None:
                    if set_all:
                        bp = language['ef_throw'](self.target)
                        self.exc_breakpoints.add(bp.GetID())
                    if set_uncaught:
                        bp = language['ef_uncaught'](self.target)
                        self.exc_breakpoints.add(bp.GetID())
                else:
                    self.console_msg('Unknown source language: %s' % lang)

    def DEBUG_configurationDone(self, args):
        try:
            self.pre_launch()
            result = self.do_launch(self.launch_args)
            # do_launch is asynchronous so we need to send its result
            self.send_response(self.launch_args['response'], result)
        except Exception as e:
            self.send_response(self.launch_args['response'], e)
        # Make sure VSCode knows if the process was initially stopped.
        if self.process is not None and self.process.is_stopped:
            self.notify_target_stopped(None)

    def DEBUG_pause(self, args):
        error = self.process.Stop()
        if error.Fail():
            raise UserError(error.GetCString())

    def DEBUG_continue(self, args):
        self.before_resume()
        error = self.process.Continue()
        if error.Fail():
            raise UserError(error.GetCString())

    def DEBUG_next(self, args):
        self.before_resume()
        tid = args['threadId']
        thread = self.process.GetThreadByID(tid)
        if not self.in_disassembly(thread.GetFrameAtIndex(0)):
            thread.StepOver()
        else:
            thread.StepInstruction(True)

    def DEBUG_stepIn(self, args):
        self.before_resume()
        tid = args['threadId']
        thread = self.process.GetThreadByID(tid)
        if not self.in_disassembly(thread.GetFrameAtIndex(0)):
            thread.StepInto()
        else:
            thread.StepInstruction(False)

    def DEBUG_stepOut(self, args):
        self.before_resume()
        tid = args['threadId']
        thread = self.process.GetThreadByID(tid)
        thread.StepOut()

    def DEBUG_stepBack(self, args):
        self.show_disassembly = 'always' # Reverse line-step is not supported (yet?)
        tid = args['threadId']
        self.reverse_exec([
            'process plugin packet send Hc%x' % tid, # select thread
            'process plugin packet send bs', # reverse-step
            'process plugin packet send bs', # reverse-step - so we can forward step
            'stepi']) # forward-step - to refresh LLDB's cached debuggee state

    def DEBUG_reverseContinue(self, args):
        self.reverse_exec([
            'process plugin packet send bc', # reverse-continue
            'process plugin packet send bs', # reverse-step
            'stepi']) # forward-step

    def reverse_exec(self, commands):
        interp = self.debugger.GetCommandInterpreter()
        result = lldb.SBCommandReturnObject()
        for command in commands:
            interp.HandleCommand(command, result)
            if not result.Succeeded():
                self.console_msg(result.GetError())
                return

    def DEBUG_threads(self, args):
        threads = []
        for thread in self.process:
            index = thread.GetIndexID()
            tid = thread.GetThreadID()
            display = '%d: tid=%d' % (index, tid)
            threads.append({ 'id': tid, 'name': display })
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
            stack_frame = { 'id': self.var_refs.create(frame, (thread.GetThreadID(), i), None) }
            fn_name = frame.GetFunctionName()
            if fn_name is None:
                fn_name = str(frame.GetPCAddress())
            stack_frame['name'] = fn_name

            if not self.in_disassembly(frame):
                le = frame.GetLineEntry()
                if le.IsValid():
                    fs = le.GetFileSpec()
                    # VSCode gets confused if the path contains funky stuff like a double-slash
                    full_path = self.map_path_to_local(fs.fullpath)
                    stack_frame['source'] = { 'name': fs.basename, 'path': full_path }
                    stack_frame['line'] = le.GetLine()
                    stack_frame['column'] = le.GetColumn()
            else:
                pc_sbaddr = frame.GetPCAddress()
                pc_addr = pc_sbaddr.GetLoadAddress(self.target)
                dasm = disassembly.find(self.disassembly_by_addr, pc_addr)
                if dasm is None:
                    log.info('Creating new disassembly for %x', pc_addr)
                    dasm = disassembly.Disassembly(pc_sbaddr, self.target)
                    disassembly.insert(self.disassembly_by_addr, dasm)
                    dasm.source_ref = self.disassembly_by_handle.create(dasm)
                stack_frame['source'] = dasm.get_source_ref()
                stack_frame['line'] = dasm.line_num_by_address(pc_addr)
                stack_frame['column'] = 0

            if not frame.GetLineEntry().IsValid():
                stack_frame['presentationHint'] = 'subtle' # No line debug info.

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
        container_info = self.var_refs.get_vpath(container_handle)
        if container_info is None:
            log.error('Invalid variables reference: %d', container_handle)
            return

        container, container_vpath = container_info
        container_name = None
        variables = collections.OrderedDict()
        if isinstance(container, lldb.SBFrame):
            # args, locals, statics, in_scope_only
            vars_iter = SBValueListIter(container.GetVariables(True, True, False, True))
            # Check if we have a return value from the last called function (usually after StepOut).
            ret_val = container.GetThread().GetStopReturnValue()
            if ret_val.IsValid():
                name, value, dtype, handle = self.parse_var(ret_val, self.global_format, container_handle)
                name = '[return value]'
                variable = { 'name': name, 'value': value, 'type': dtype, 'variablesReference': handle }
                variables[name] = variable
        elif isinstance(container, StaticsScope):
            vars_iter = SBValueListIter(container.frame.GetVariables(False, False, True, True))
        elif isinstance(container, RegistersScope):
            vars_iter = SBValueListIter(container.frame.GetRegisters())
        elif isinstance(container, lldb.SBValue):
            value_type = container.GetValueType()
            if value_type != lldb.eValueTypeRegisterSet: # Registers are addressed by name, without parent reference.
                for segment in container_vpath[1:]: # First element is the scope object.
                    container_name = compose_eval_name(container_name, segment)
            vars_iter = SBValueChildrenIter(container)

        for var in vars_iter:
            name, value, dtype, handle = self.parse_var(var, self.global_format, container_handle)
            if name is None: # Sometimes LLDB returns junk entries with empty names and values
                continue
            variable = { 'name': name, 'value': value, 'type': dtype, 'variablesReference': handle,
                         'evaluateName': compose_eval_name(container_name, name) }
            # Ensure proper variable shadowing: if variable of the same name had already been added,
            # remove it and insert the new instance at the end.
            if name in variables:
                del variables[name]
            variables[name] = variable

        variables = list(variables.values())

        # If this node was synthetic (i.e. a product of a visualizer),
        # append [raw] pseudo-child, which can be expanded to show the raw view.
        if isinstance(container, lldb.SBValue) and container.IsSynthetic():
            handle = self.var_refs.create(container.GetNonSyntheticValue(), '[raw]', container_handle)
            variable = { 'name': '[raw]', 'value': container.GetTypeName(), 'variablesReference': handle }
            variables.append(variable)

        return { 'variables': variables }

    def DEBUG_completions(self, args):
        interp = self.debugger.GetCommandInterpreter()
        text = to_lldb_str(args['text'])
        column = int(args['column'])
        matches = lldb.SBStringList()
        result = interp.HandleCompletion(text, column-1, 0, -1, matches)
        targets = []
        for match in matches:
            targets.append({ 'label': match })
        return { 'targets': targets }

    def DEBUG_evaluate(self, args):
        if self.process is None: # Sometimes VSCode sends 'evaluate' before launching a process...
            log.error('evaluate without a process')
            return { 'result': '' }
        context = args.get('context')
        expr = to_lldb_str(args['expression'])
        if context in ['watch', 'hover', None]: # 'Copy Value' in Locals does not send context.
            return self.evaluate_expr(args, expr)
        elif expr.startswith('?'): # "?<expr>" in 'repl' context
            return self.evaluate_expr(args, expr[1:])
        # Else evaluate as debugger command
        frame = self.var_refs.get(args.get('frameId'), None)
        result = self.execute_command_in_frame(expr, frame)
        output = result.GetOutput() if result.Succeeded() else result.GetError()
        # returning output as result would display all line breaks as '\n'
        self.console_msg(output)
        return { 'result': '' }

    def execute_command_in_frame(self, command, frame):
        # set up evaluation context
        if frame is not None:
            self.set_selected_frame(frame)
        # evaluate
        interp = self.debugger.GetCommandInterpreter()
        result = lldb.SBCommandReturnObject()
        if '\n' not in command:
            interp.HandleCommand(to_lldb_str(command), result)
        else:
            # multiline command
            tmp_file = tempfile.NamedTemporaryFile()
            log.info('multiline command in %s', tmp_file.name)
            tmp_file.write(to_lldb_str(command))
            tmp_file.flush()
            filespec = lldb.SBFileSpec()
            filespec.SetDirectory(os.path.dirname(tmp_file.name))
            filespec.SetFilename(os.path.basename(tmp_file.name))
            context = lldb.SBExecutionContext(frame)
            options = lldb.SBCommandInterpreterRunOptions()
            interp.HandleCommandsFromFile(filespec, context, options, result)
        sys.stdout.flush()
        return result

    # Selects a frame in LLDB context.
    def set_selected_frame(self, frame):
        thread = frame.GetThread()
        thread.SetSelectedFrame(frame.GetFrameID())
        process = thread.GetProcess()
        process.SetSelectedThread(thread)
        lldb.frame = frame
        lldb.thread = thread
        lldb.process = process
        lldb.target = process.GetTarget()

    def evaluate_expr(self, args, expr):
        frame_id = args.get('frameId') # May be null
        # parse format suffix, if any
        format = self.global_format
        for suffix, fmt in self.format_codes:
            if expr.endswith(suffix):
                format = fmt
                expr = expr[:-len(suffix)]
                break

        context = args.get('context')
        saved_stderr = sys.stderr
        if context == 'hover':
            # Because hover expressions may be invalid through no user's fault,
            # we want to suppress any stderr output resulting from their evaluation.
            sys.stderr = None
        try:
           frame = self.var_refs.get(frame_id, None)
           result = self.evaluate_expr_in_frame(expr, frame)
        finally:
            sys.stderr = saved_stderr

        if isinstance(result, lldb.SBError):
            error_message = result.GetCString()
            if context == 'repl':
                self.console_msg(error_message)
                return None
            else:
                raise UserError(error_message.replace('\n', '; '), no_console=True)
        elif isinstance(result, expressions.Value):
            _, value, dtype, handle = self.parse_var(expressions.Value.unwrap(result), format)
            return { 'result': value, 'type': dtype, 'variablesReference': handle }
        else: # Some Python value
            return { 'result': str(result), 'variablesReference': 0 }

    # Evaluating these names causes LLDB exit, not sure why.
    pyeval_globals = { 'exit':None, 'quit':None, 'globals':None }

    # Evaluates expr in the context of frame (or in global context if frame is None)
    # Returns expressions.Value or SBValue on success, SBError on failure.
    def evaluate_expr_in_frame(self, expr, frame):
        if expr.startswith('/nat '):
            # Using LLDB native evaluator
            expr = expr[5:]
            if frame is not None:
                result = frame.EvaluateExpression(expr) # In frame context
            else:
                result = self.target.EvaluateExpression(expr) # In global context
            error = result.GetError()
            if error.Success():
                return result
            else:
                return error
        else:
            if frame is None: # Use the currently selected frame
                frame = self.process.GetSelectedThread().GetSelectedFrame()

            if expr.startswith('/py '):
                # Python expression
                expr = expressions.preprocess_varsubsts(expr[4:])
                self.set_selected_frame(frame)
                import __main__
                eval_globals = getattr(__main__, self.debugger.GetInstanceName() + '_dict')
                eval_globals['__frame_vars'] = expressions.PyEvalContext(frame)
                eval_locals = {}
            else:
                # Simple expression
                expr = expressions.preprocess(expr)
                eval_globals = self.pyeval_globals
                eval_locals = expressions.PyEvalContext(frame)

            try:
                log.debug('Evaluating %s', expr)
                return eval(expr, eval_globals, eval_locals)
            except Exception as e:
                log.debug('Evaluation error: %s', traceback.format_exc())
                error = lldb.SBError()
                error.SetErrorString(to_lldb_str(str(e)))
                return error

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
        # Synthetic vars will at least have the [raw] child.
        if var.GetNumChildren() > 0 or var.IsSynthetic():
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
        expressions.analyze(var)
        var.SetFormat(format)
        is_pointer = var.GetType().GetTypeClass() in [lldb.eTypeClassPointer,
                                                      lldb.eTypeClassReference]
        if is_pointer and format == lldb.eFormatDefault:
            # For pointers and references, when format is eFormatDefault, fall through
            # to var.GetSummary() below which will extract the summary of the object it points to.
            value = None
        else:
            value = var.GetValue()

        if value is None:
            value = var.GetSummary()
            if value is not None:
                value = value.replace('\n', '') # VSCode won't display line breaks
        if value is not None:
            value = from_lldb_str(value)
        return value

    # Clears out cached state that become invalid once debuggee resumes.
    def before_resume(self):
        self.var_refs.reset()

    def DEBUG_setVariable(self, args):
        container = self.var_refs.get(args['variablesReference'])
        if container is None:
            raise Exception('Invalid variables reference')

        name = to_lldb_str(args['name'])
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
        if not var.SetValueFromCString(to_lldb_str(args['value']), error):
            raise UserError(error.GetCString())
        return { 'value': self.get_var_value(var, self.global_format) }

    def DEBUG_disconnect(self, args):
        if self.launch_args is not None:
            self.exec_commands(self.launch_args.get('exitCommands'))
        if self.process:
            if args.get('terminateDebuggee', self.process_launched):
                self.process.Kill()
            else:
                self.process.Detach()
        self.process = None
        self.target = None
        self.terminal = None
        self.listener_handler_token = None
        self.event_loop.stop()

    def DEBUG_test(self, args):
        self.console_msg('TEST\n')

    def DEBUG_showDisassembly(self, args):
        value = args.get('value', 'toggle')
        if value == 'toggle':
            self.show_disassembly = 'auto' if self.show_disassembly != 'auto' else 'always'
        else:
            self.show_disassembly = value
        self.refresh_client_display()

    def DEBUG_displayFormat(self, args):
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

    def DEBUG_provideContent(self, args):
        return { 'content': self.provide_content(args['uri']) }

    # Fake a target stop to force VSCode to refresh the display
    def refresh_client_display(self):
        thread_id = self.process.GetSelectedThread().GetThreadID()
        self.send_event('continued', { 'threadId': thread_id,
                                       'allThreadsContinued': True })
        self.send_event('stopped', { 'reason': 'mode switch',
                                     'threadId': thread_id,
                                     'allThreadsStopped': True })

    # handles messages from VSCode debug client
    def handle_message(self, message):
        if message is None:
            # Client connection lost; treat this the same as a normal disconnect.
            self.DEBUG_disconnect(None)
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
                    self.send_response(response, result)
                except Exception as e:
                    self.send_response(response, e)
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
            tb = traceback.format_exc()
            log.error('Internal debugger error:\n%s', tb)
            self.console_msg('Internal debugger error:\n' + tb)
            msg = 'Internal debugger error: ' + str(result)
            response['success'] = False
            response['body'] = { 'error': { 'id': 0, 'format': msg, 'showUser': True } }
        else:
            assert False, "Invalid result type: %s" % result
        self.send_message(response)

    # Send a request to VSCode. When response is received, on_complete(True, request.body)
    # will be called on success, or on_complete(False, request.message) on failure.
    def send_request(self, command, args, on_complete):
        request = { 'type': 'request', 'seq': self.request_seq, 'command': command,
                    'arguments': args }
        self.pending_requests[self.request_seq] = on_complete
        self.request_seq += 1
        self.send_message(request)

    # Handles debugger notifications
    def handle_debugger_event(self, event):
        if lldb.SBProcess.EventIsProcessEvent(event):
            ev_type = event.GetType()
            if ev_type == lldb.SBProcess.eBroadcastBitStateChanged:
                state = lldb.SBProcess.GetStateFromEvent(event)
                if state == lldb.eStateStopped:
                    if not lldb.SBProcess.GetRestartedFromEvent(event):
                        self.notify_target_stopped(event)
                elif state == lldb.eStateRunning:
                    self.send_event('continued', { 'threadId': 0, 'allThreadsContinued': True })
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
        self.update_threads()
        event = { 'allThreadsStopped': True } # LLDB always stops all threads
        # Find the thread that caused this stop
        stopped_thread = None
        # Check the currently selected thread first
        thread = self.process.GetSelectedThread()
        if thread is not None and thread.IsValid():
            stop_reason = thread.GetStopReason()
            if stop_reason != lldb.eStopReasonInvalid and stop_reason != lldb.eStopReasonNone:
                stopped_thread = thread
        # Fall back to scanning all threads in process
        if stopped_thread is None:
            for thread in self.process:
                stop_reason = thread.GetStopReason()
                if stop_reason != lldb.eStopReasonInvalid and stop_reason != lldb.eStopReasonNone:
                    stopped_thread = thread
                    self.process.SetSelectedThread(stopped_thread)
                    break
        # Analyze stop reason
        if stopped_thread is not None:
            if stop_reason == lldb.eStopReasonBreakpoint:
                bp_id = thread.GetStopReasonDataAtIndex(0)
                if bp_id in self.exc_breakpoints:
                    stop_reason_str = 'exception'
                else:
                    stop_reason_str = 'breakpoint'
            elif stop_reason == lldb.eStopReasonTrace or stop_reason == lldb.eStopReasonPlanComplete:
                stop_reason_str = 'step'
            else:
                # Print stop details for these types
                if stop_reason == lldb.eStopReasonWatchpoint:
                    stop_reason_str = 'watchpoint'
                elif stop_reason == lldb.eStopReasonSignal:
                    stop_reason_str = 'signal'
                elif stop_reason == lldb.eStopReasonException:
                    stop_reason_str = 'exception'
                else:
                    stop_reason_str = 'unknown'
                description = stopped_thread.GetStopDescription(100)
                self.console_msg('Stop reason: %s' % description)
                event['text'] = description

            event['reason'] = stop_reason_str
            event['threadId'] = stopped_thread.GetThreadID()

        self.send_event('stopped', event)

    # Notify VSCode about target threads that started or exited since the last stop.
    def update_threads(self):
        threads = set()
        for thread in self.process:
            threads.add(thread.GetThreadID())
        started = threads - self.known_threads
        exited = self.known_threads - threads
        for thread_id in exited:
            self.send_event('thread', { 'threadId': thread_id, 'reason': 'exited' })
        for thread_id in started:
            self.send_event('thread', { 'threadId': thread_id, 'reason': 'started' })
        self.known_threads = threads

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
        event_type = lldb.SBBreakpoint.GetBreakpointEventTypeFromEvent(event)
        if event_type == lldb.eBreakpointEventTypeLocationsResolved:
            bp = lldb.SBBreakpoint.GetBreakpointFromEvent(event)
            bp_info = self.make_bp_resp(bp)
            self.send_event('breakpoint', { 'reason': 'changed', 'breakpoint': bp_info })

    def handle_debugger_output(self, output):
        self.console_msg(output)

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

    def map_path_to_local(self, path):
        path = os.path.normpath(path)
        path_normcased = os.path.normcase(path)
        for remote_prefix, local_prefix in self.launch_args.get("sourceMap", {}).items():
            if path_normcased.startswith(os.path.normcase(remote_prefix)):
                # This assumes that os.path.normcase does not change string length,
                # but we want to preserve the original path casing...
                return os.path.normpath(local_prefix + path[len(remote_prefix):])
        return path

    # Ask VSCode extension to display HTML content.
    def display_html(self, body):
        self.send_event('displayHtml', body)

def on_breakpoint_hit(frame, bp_loc, internal_dict):
    return DebugSession.current.should_stop_on_bp(bp_loc.GetBreakpoint().GetID(), frame, internal_dict)

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

def SBValueListIter(val_list):
    get_value = val_list.GetValueAtIndex
    for i in xrange(val_list.GetSize()):
        yield get_value(i)

def SBValueChildrenIter(val):
    get_value = val.GetChildAtIndex
    for i in xrange(val.GetNumChildren(MAX_VAR_CHILDREN)):
        yield get_value(i)

def opt_lldb_str(s):
    return to_lldb_str(s) if s != None else None

def same_path(path1, path2):
    return os.path.normcase(os.path.normpath(path1)) == os.path.normcase(os.path.normpath(path2))

def compose_eval_name(container, var_name):
    if container is None:
        return var_name
    elif var_name.startswith('['):
        return container + var_name
    else:
        return container + '.' + var_name

languages = {
    'rust': {
        'ef_throw': lambda target: target.BreakpointCreateByName('rust_panic'),
        'ef_uncaught': lambda target: target.BreakpointCreateByName('abort'),
    },
    'cpp': {
        'ef_throw': lambda target: target.BreakpointCreateForException(lldb.eLanguageTypeC_plus_plus, False, True),
        'ef_uncaught': lambda target: target.BreakpointCreateByName('terminate'),
    }
}
