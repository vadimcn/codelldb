import sys
import logging
import os.path
import shlex
import traceback
import collections
import tempfile
import re
import fnmatch
import json
import time
import lldb
from . import expressions
from . import debugevents
from . import disassembly
from . import handles
from . import terminal
from . import mem_limit
from . import PY2, is_string, from_lldb_str, to_lldb_str, xrange

log = logging.getLogger('debugsession')
log.info('Imported')

# The maximum number of children we'll retrieve for a container value.
# This is to cope with the not yet initialized objects whose length fields contain garbage.
MAX_VAR_CHILDREN = 10000

# When None is a valid dictionary entry value, we need some other value to designate missing entries.
MISSING = ()

# Expression types
SIMPLE = 'simple'
PYTHON = 'python'
NATIVE = 'native'

# Breakpoint types
SOURCE = 'source'
FUNCTION = 'function'
ASSEMBLY = 'assembly'
EXCEPTION = 'exception'

class DebugSession:

    def __init__(self, parameters, event_loop, send_message):
        DebugSession.current = self
        self.parameters = parameters
        self.event_loop = event_loop
        self.send_message = send_message
        self.var_refs = handles.HandleTree()
        self.line_breakpoints = dict() # { file_id : { line : breakpoint_id } }
        self.fn_breakpoints = dict() # { fn_name : breakpoint_id }
        self.breakpoints = dict() # { breakpoint_id : BreakpointInfo }
        self.target = None
        self.process = None
        self.terminal = None
        self.launch_args = None
        self.process_launched = False
        self.disassembly = None # disassembly.AddressSpace; need SBTarget to create
        self.request_seq = 1
        self.pending_requests = {} # { seq : on_complete }
        self.known_threads = set()
        self.global_format = lldb.eFormatDefault
        self.show_disassembly = 'auto' # never | auto | always
        self.deref_pointers = True
        self.container_summary = True
        self.suppress_missing_sources = self.parameters.get('suppressMissingSourceFiles', True)
        self.evaluation_timeout = self.parameters.get('evaluationTimeout', 5)

    def DEBUG_initialize(self, args):
        init_hook = self.parameters.get('init_hook')
        if init_hook: init_hook()

        self.line_offset = 0 if args.get('linesStartAt1', True) else 1
        self.col_offset = 0 if args.get('columnsStartAt1', True) else 1

        self.debugger = lldb.debugger if lldb.debugger else lldb.SBDebugger.Create()
        log.info('LLDB version: %s', self.debugger.GetVersionString())
        self.debugger.SetAsync(True)
        self.debugger.HandleCommand('script import adapter, debugger')
        import __main__
        self.session_dict = getattr(__main__, self.debugger.GetInstanceName() + '_dict')

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

        src_langs = self.parameters.get('sourceLanguages', ['cpp'])
        exc_filters = [{ 'filter':filter, 'label':label, 'default':default }
                       for filter, label, default in self.get_exception_filters(src_langs)]
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
            'supportsLogPoints': True,
            'supportsStepBack': self.parameters.get('reverseDebugging', False),
            'exceptionBreakpointFilters': exc_filters,
        }

    def DEBUG_launch(self, args):
        self.update_display_settings(args.get('_adapterSettings'))
        self.init_source_map(args)
        if args.get('request') == 'custom' or args.get('custom', False):
            return self.custom_launch(args)
        self.exec_commands(args.get('initCommands'))
        self.target = self.create_target(args)
        self.disassembly = disassembly.AddressSpace(self.target)
        self.send_event('initialized', {})
        # defer actual launching till configurationDone request, so that
        # we can receive and set initial breakpoints before the target starts running
        self.do_launch = self.complete_launch
        self.launch_args = args
        return AsyncResponse

    def complete_launch(self, args):
        self.exec_commands(args.get('preRunCommands'))
        flags = 0
        # argumetns
        target_args = args.get('args', None)
        if target_args is not None:
            if is_string(target_args):
                target_args = shlex.split(target_args)
            target_args = [to_lldb_str(arg) for arg in target_args]
        # environment
        environment = dict(os.environ)
        environment.update(args.get('env', {}))
        # convert dict to a list of 'key=value' strings
        envp = [to_lldb_str('%s=%s' % pair) for pair in environment.items()]
        # stdio
        stdio, extra_flags = self.configure_stdio(args)
        flags |= extra_flags
        flags |= lldb.eLaunchFlagDisableASLR
        # working directory
        work_dir = opt_lldb_str(args.get('cwd', None))
        stop_on_entry = args.get('stopOnEntry', False)
        program = to_lldb_str(args['program'])
        # launch!
        args_str = ' '.join(target_args) if target_args is not None else ''
        self.console_msg('Launching %s %s' % (program, args_str))
        log.debug('Launching: program=%r, args=%r, envp=%r, stdio=%r, cwd=%r, flags=0x%X',
            program, target_args, envp, stdio, work_dir, flags)
        error = lldb.SBError()
        self.process = self.target.Launch(self.event_listener,
            target_args, envp, stdio[0], stdio[1], stdio[2],
            work_dir, flags, stop_on_entry, error)
        if not error.Success():
            self.send_event('terminated', {})
            err_msg = error.GetCString()
            if self.target.GetPlatform().GetFilePermissions(work_dir) == 0:
                err_msg += '\n\nPossible cause: the working directory "%s" is missing or inaccessible.' % work_dir
            raise UserError(err_msg)

        assert self.process.IsValid()
        self.process_launched = True
        self.exec_commands(args.get('postRunCommands'))

    def DEBUG_attach(self, args):
        self.update_display_settings(args.get('_adapterSettings'))
        self.init_source_map(args)
        pid = args.get('pid', None)
        program = args.get('program', None)
        if pid is None and program is None:
            raise UserError('Either \'program\' or \'pid\' is required for attach.')
        self.exec_commands(args.get('initCommands'))
        self.target = self.debugger.CreateTarget('') # A dummy target, will be initialized once we attach
        self.disassembly = disassembly.AddressSpace(self.target)
        self.send_event('initialized', {})
        self.do_launch = self.complete_attach
        self.launch_args = args
        return AsyncResponse

    def complete_attach(self, args):
        self.exec_commands(args.get('preRunCommands'))
        error = lldb.SBError()
        pid = args.get('pid', None)
        if pid is not None:
            if is_string(pid): pid = int(pid)
            self.console_msg('Attaching to pid=%d' % pid)
            self.process = self.target.AttachToProcessWithID(self.event_listener, pid, error)
        else:
            program = to_lldb_str(args['program'])
            program = self.find_executable(program)
            self.console_msg('Attaching to %s' % program)
            waitFor = args.get('waitFor', False)
            self.process = self.target.AttachToProcessWithName(self.event_listener, program, waitFor, error)
        if not error.Success():
            self.diagnose_attach_failure(error)
            raise UserError(error.GetCString())
        assert self.process.IsValid()
        self.process_launched = False
        if not args.get('stopOnEntry', False):
            self.process.Continue()
        self.exec_commands(args.get('postRunCommands'))

    def diagnose_attach_failure(self, error):
        if 'linux' in sys.platform:
            ptrace_scope_path = '/proc/sys/kernel/yama/ptrace_scope'
            try:
                value = int(open(ptrace_scope_path, 'r').read().strip())
                if value != 0:
                    if value == 1:
                        message = '- your system configuration restricts process attachment to child processes only.'
                    elif value == 2:
                        message = '- your system configuration restricts debugging to privileged processes only.'
                    elif value == 3:
                        message = '- your system configuration does not allow debugging.'
                    else:
                        message = '(unknown value).'
                    self.console_msg('Warning: The value of %s is %d %s' % (ptrace_scope_path, value, message))
                    self.console_msg('For more information see: https://en.wikipedia.org/wiki/Ptrace, https://www.kernel.org/doc/Documentation/security/Yama.txt')
            except Error:
                pass

    def custom_launch(self, args):
        create_target = args.get('targetCreateCommands') or args.get('initCommands')
        self.exec_commands(create_target)
        self.target = self.debugger.GetSelectedTarget()
        if not self.target.IsValid():
            self.console_err('Warning: target is invalid after running "targetCreateCommands".')
        self.target.GetBroadcaster().AddListener(self.event_listener, lldb.SBTarget.eBroadcastBitBreakpointChanged)
        self.disassembly = disassembly.AddressSpace(self.target)
        self.send_event('initialized', {})
        self.do_launch = self.complete_custom_launch
        self.launch_args = args
        return AsyncResponse

    def complete_custom_launch(self, args):
        log.info('Custom launching...')
        create_process = args.get('processCreateCommands') or args.get('preRunCommands')
        self.exec_commands(create_process)
        self.process = self.target.GetProcess()
        if not self.process.IsValid():
            self.console_err('Warning: process is invalid after running "processCreateCommands".')
        self.process.GetBroadcaster().AddListener(self.event_listener, 0xFFFFFF)
        self.process_launched = False

    def create_target(self, args):
        program = args.get('program')
        if program is not None:
            load_dependents = not args.get('noDebug', False)
            error = lldb.SBError()
            program = self.find_executable(program)
            target = self.debugger.CreateTarget(to_lldb_str(program), None, None, load_dependents, error)
            if not error.Success():
                raise UserError('Could not initialize debug target: ' + error.GetCString())
        else:
            if args['request'] == 'launch':
                raise UserError('\'program\' property is required for launch.')
            target = self.debugger.CreateTarget('') # OK if attaching by pid
        target.GetBroadcaster().AddListener(self.event_listener,
            lldb.SBTarget.eBroadcastBitBreakpointChanged | lldb.SBTarget.eBroadcastBitModulesLoaded)
        return target

    def find_executable(self, program):
        # On Windows, also try program + '.exe'
        if 'win32' in sys.platform:
            if not os.path.isfile(program):
                program_exe = program + '.exe'
                if os.path.isfile(program_exe):
                    return program_exe
        return program

    def pre_launch(self):
        formatters = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'formatters')
        for name in os.listdir(formatters):
            file_path = os.path.join(formatters, name)
            if name.endswith('.py') or os.path.isdir(file_path):
                self.exec_commands(['command script import \'%s\'' % file_path])

    def init_source_map(self, args):
        source_map = args.get('sourceMap')
        if source_map is None:
            return
        value = []
        for remote_prefix, local_prefix in source_map.items():
            value.append(remote_prefix)
            value.append(local_prefix)
        value = '"' + '" "'.join([v.replace('\\', '\\\\').replace('"', '\\"') for v in value]) + '"'
        lldb.SBDebugger.SetInternalVariable('target.source-map', to_lldb_str(value), self.debugger.GetInstanceName())

    def exec_commands(self, commands):
        if commands is not None:
            interp = self.debugger.GetCommandInterpreter()
            result = lldb.SBCommandReturnObject()
            for command in commands:
                interp.HandleCommand(to_lldb_str(command), result)
                sys.stdout.flush()
                if result.Succeeded():
                    self.console_msg(result.GetOutput())
                else:
                    self.console_err(result.GetError())
            sys.stdout.flush()

    def configure_stdio(self, args):
        stdio = args.get('stdio', None)
        if isinstance(stdio, dict): # Flatten it into a list
            stdio = [stdio.get('stdin', MISSING),
                     stdio.get('stdout', MISSING),
                     stdio.get('stderr', MISSING)]
        elif stdio is None or is_string(stdio):
            stdio = [stdio] * 3
        elif isinstance(stdio, list):
            stdio.extend([MISSING] * (3-len(stdio))) # pad up to 3 items
        else:
            raise UserError('stdio must be either a string, a list or an object')
        # replace all missing's with the previous stream's value
        for i in range(0, len(stdio)):
            if stdio[i] is MISSING:
                stdio[i] = stdio[i-1] if i > 0 else None
        # Map '*' to None and convert strings to ASCII
        stdio = [to_lldb_str(s) if s not in ['*', None] else None for s in stdio]
        # open a new terminal window if needed
        extra_flags = 0
        if None in stdio:
            term_type = args.get('terminal', 'console')
            if 'win32' not in sys.platform:
                if term_type in ['integrated', 'external']:
                    title = 'Debug - ' + args.get('name', '?')
                    self.terminal = terminal.create(
                        lambda args: self.spawn_vscode_terminal(kind=term_type, args=args, title=title))
                    term_fd = to_lldb_str(self.terminal.tty)
                else:
                    term_fd = None # that'll send them to VSCode debug console
            else: # Windows
                no_console = 'false' if term_type == 'external' else 'true'
                os.environ['LLDB_LAUNCH_INFERIORS_WITHOUT_CONSOLE'] = no_console
                term_fd = None # no other options on Windows
            stdio = [term_fd if s is None else to_lldb_str(s) for s in stdio]
        return stdio, extra_flags

    def spawn_vscode_terminal(self, kind, args=[], cwd='', env=None, title='Debuggee'):
        if kind == 'integrated':
            args[0] = '\n' + args[0] # Extra end of line to flush junk

        self.send_request('runInTerminal', {
            'kind': kind, 'cwd': cwd, 'args': args, 'env': env, 'title': title
        }, lambda ok, body: None)

    def disable_bp_events(self):
        self.target.GetBroadcaster().RemoveListener(self.event_listener, lldb.SBTarget.eBroadcastBitBreakpointChanged)

    def enable_bp_events(self):
        self.target.GetBroadcaster().AddListener(self.event_listener, lldb.SBTarget.eBroadcastBitBreakpointChanged)

    def DEBUG_setBreakpoints(self, args):
        if self.launch_args.get('noDebug', False):
            return
        try:
            self.disable_bp_events()

            source = args['source']
            req_bps = args['breakpoints']
            req_bp_lines = [req['line'] for req in req_bps]

            dasm = None
            adapter_data = None
            file_id = None  # File path or a source reference.

            # We need to handle three cases:
            # - `source` has `sourceReference` attribute, which indicates breakpoints in disassembly,
            #   for which we had already created ephemeral file in the current debug session.
            # - `source` has `adapterData` attribute (but no `sourceReference`), which indicates
            #   disassembly breakpoints that existed in earlier debug session.  We attempt to
            #   re-create the Disassembly objects using `adapterData`.
            # - Otherwise, `source` refers to a regular source file.
            source_ref = source.get('sourceReference')
            if source_ref:
                dasm = self.disassembly.get_by_handle(source_ref)
                # Sometimes VSCode hands us stale source refs, so this lookup is not guarantted to succeed.
                if dasm:
                    file_id = dasm.source_ref
                    # Construct adapterData for this source, so we can recover breakpoint addresses
                    # in subsequent debug sessions.
                    line_addresses = { str(line) : dasm.address_by_line_num(line) for line in req_bp_lines }
                    source['adapterData'] = { 'start': dasm.start_address, 'end': dasm.end_address,
                                            'lines': line_addresses }
            if not dasm:
                adapter_data = source.get('adapterData')
                file_id = from_lldb_str(source.get('path'))

            assert file_id is not None

            # Existing breakpints indexed by line number.
            file_bps = self.line_breakpoints.setdefault(file_id, {})

            # Clear existing breakpints that were removed
            for line, bp_id in list(file_bps.items()):
                if line not in req_bp_lines:
                    self.target.BreakpointDelete(bp_id)
                    del file_bps[line]
                    del self.breakpoints[bp_id]
            # Added or updated breakpoints
            if dasm:
                result = self.set_asm_breakpoints(file_bps, req_bps,
                    lambda line: dasm.address_by_line_num(line), source, adapter_data, True)
            elif adapter_data:
                line_addresses = adapter_data['lines']
                result = self.set_asm_breakpoints(file_bps, req_bps,
                    lambda line: line_addresses[str(line)], source, adapter_data, False)
            else:
                result = self.set_source_breakpoints(file_bps, req_bps, file_id)
            return { 'breakpoints': result }
        finally:
            self.enable_bp_events()

    def set_source_breakpoints(self, file_bps, req_bps, file_path):
        result = []
        file_name = os.path.basename(file_path)
        for req in req_bps:
            line = req['line']
            bp_id = file_bps.get(line, None)
            if bp_id: # Existing breakpoint
                bp = self.target.FindBreakpointByID(bp_id)
                bp_resp = { 'id': bp_id, 'verified': True }
            else:  # New breakpoint
                bp = self.target.BreakpointCreateByLocation(to_lldb_str(file_path), line)
                bp_id = bp.GetID()
                bp_info = BreakpointInfo(bp_id, SOURCE)
                bp_info.file_path = file_path
                bp_info.line = line
                self.breakpoints[bp_id] = bp_info
                for bp_loc in bp:
                    if bp_loc.IsResolved():
                        bp_info.verified = True
                bp_resp = self.make_bp_resp(bp, bp_info)
            self.init_bp_actions(bp, req)
            file_bps[line] = bp_id
            result.append(bp_resp)
        return result

    def set_asm_breakpoints(self, file_bps, req_bps, addr_from_line, source, adapter_data, verified):
        result = []
        for req in req_bps:
            line = req['line']
            bp_id = file_bps.get(line, None)
            if bp_id: # Existing breakpoint
                bp = self.target.FindBreakpointByID(bp_id)
                bp_resp = { 'id': bp_id, 'source': source, 'verified': verified }
            else:  # New breakpoint
                addr = addr_from_line(line)
                bp = self.target.BreakpointCreateByAddress(addr)
                bp_id = bp.GetID()

                bp_info = BreakpointInfo(bp_id, ASSEMBLY)
                bp_info.address = addr
                bp_info.adapter_data = adapter_data
                self.breakpoints[bp_id] = bp_info

                bp_resp = { 'id': bp_id }
                bp_resp['source'] = source
                bp_resp['line'] = line
                bp_resp['verified'] = verified
            self.init_bp_actions(bp, req)
            file_bps[line] = bp_id
            result.append(bp_resp)
        return result

    def DEBUG_setFunctionBreakpoints(self, args):
        if self.launch_args.get('noDebug', False):
            return
        try:
            self.disable_bp_events()

            result = []
            # Breakpoint requests indexed by function name
            req_bps = args['breakpoints']
            req_bp_names = [req['name'] for req in req_bps]
            # Existing breakpints that were removed
            for name, bp_id in list(self.fn_breakpoints.items()):
                if name not in req_bp_names:
                    self.target.BreakpointDelete(bp_id)
                    del self.fn_breakpoints[name]
                    del self.breakpoints[bp_id]
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
                    bp_id = bp.GetID()
                    self.fn_breakpoints[name] = bp_id
                    self.breakpoints[bp_id] = BreakpointInfo(bp_id, FUNCTION)
                    self.init_bp_actions(bp, req)
                else:
                    bp = self.target.FindBreakpointByID(bp_id)

                verified = bp.GetNumResolvedLocations() > 0
                result.append({ 'id': bp_id, 'verified': verified })
            return { 'breakpoints': result }
        finally:
            self.enable_bp_events()

    # Sets up breakpoint stopping condition
    def init_bp_actions(self, bp, req):
        bp_info = self.breakpoints[bp.GetID()]

        if bp_info.condition or bp_info.ignore_count:
            bp_info.condition = None
            bp_info.ignore_count = 0
            bp.SetCondition(None)

        cond = opt_lldb_str(req.get('condition', None))
        if cond:
            ty, cond = self.get_expression_type(cond)
            if ty == NATIVE:
                bp.SetCondition(cond)
            else:
                if ty == PYTHON:
                    eval_condition = self.make_python_expression_bpcond(cond)
                else: # SIMPLE
                    eval_condition = self.make_simple_expression_bpcond(cond)

                if eval_condition:
                    bp_info.condition = eval_condition

        ignore_count_str = req.get('hitCondition', None)
        if ignore_count_str:
            try:
                bp_info.ignore_count = int(ignore_count_str)
                bp.SetIgnoreCount(bp_info.ignore_count)
            except ValueError:
                self.console_err('Could not parse ignore count as integer: %s' % ignore_count_str)

        bp_info.log_message = req.get('logMessage', None)

        bp.SetScriptCallbackFunction('adapter.debugsession.on_breakpoint_hit')

    # Compiles a python expression into a breakpoint condition evaluator
    def make_python_expression_bpcond(self, cond):
        pp_cond = expressions.preprocess_python_expr(cond)
        # Try compiling as expression first, if that fails, compile as a statement.
        error = None
        try:
            pycode = compile(pp_cond, '<breakpoint condition>', 'eval')
            is_expression = True
        except SyntaxError:
            try:
                pycode = compile(pp_cond, '<breakpoint condition>', 'exec')
                is_expression = False
            except Exception as e:
                error = e
        except Exception as e:
            error = e

        if error is not None:
            self.console_err('Could not set breakpoint condition "%s": %s' % (cond, str(error)))
            return None

        def eval_condition(bp_loc, frame, eval_globals):
            self.set_selected_frame(frame)
            hit_count = bp_loc.GetBreakpoint().GetHitCount()
            eval_locals = { 'frame': frame, 'bpno': bp_loc, 'hit_count': hit_count }
            eval_globals['__frame_vars'] = expressions.PyEvalContext(frame)
            result = eval(pycode, eval_globals, eval_locals)
            # Unconditionally continue execution if 'cond' is a statement
            return bool(result) if is_expression else False
        return eval_condition

    # Compiles a simple expression into a breakpoint condition evaluator
    def make_simple_expression_bpcond(self, cond):
        pp_cond = expressions.preprocess_simple_expr(cond)
        try:
            pycode = compile(pp_cond, '<breakpoint condition>', 'eval')
        except Exception as e:
            self.console_err('Could not set breakpoint condition "%s": %s' % (cond, str(e)))
            return None

        def eval_condition(bp_loc, frame, eval_globals):
            frame_vars = expressions.PyEvalContext(frame)
            eval_globals['__frame_vars'] = frame_vars
            return eval(pycode, eval_globals, frame_vars)
        return eval_condition

    # Create breakpoint location info for a response message.
    def make_bp_resp(self, bp, bp_info=None):
        if bp_info is None:
            bp_info = self.breakpoints.get(bp.GetID())

        breakpoint = { 'id': bp_info.id }
        if bp_info.kind == SOURCE:
            breakpoint['source'] = { 'name': os.path.basename(bp_info.file_path), 'path': bp_info.file_path }
            if bp_info.line is not None:
                breakpoint['line'] = bp_info.line
            breakpoint['verified'] = True if bp_info.verified else False
            return breakpoint
        elif bp_info.kind == ASSEMBLY:
            dasm = self.disassembly.get_by_address(bp_info.address)
            adapter_data = bp_info.adapter_data
            if not dasm and adapter_data:
                # This must be resolution of location of an assembly-level breakpoint.
                start = lldb.SBAddress(adapter_data['start'], self.target)
                end = lldb.SBAddress(adapter_data['end'], self.target)
                dasm = self.disassembly.create_from_range(start, end)
            if dasm:
                breakpoint['source'] = { 'name': dasm.source_name,
                                         'sourceReference': dasm.source_ref,
                                         'adapterData': adapter_data }
                breakpoint['line'] = dasm.line_num_by_address(bp_info.address)
                breakpoint['verified'] = True
                return breakpoint
        else: # FUNCTION or EXCEPTION
            breakpoint['verified'] = bp.GetNumResolvedLocations() > 0
            return breakpoint


    substitution_regex = re.compile('{( (?:' +
                                    expressions.nested_brackets_matcher('{', '}', 10) +
                                    '|[^}])* )}', re.X)
    def should_stop_on_bp(self, bp_loc, frame, internal_dict):
        bp = bp_loc.GetBreakpoint()
        bp_info = self.breakpoints.get(bp.GetID())
        if bp_info is None: # Something's wrong... just stop
            return True

        if bp_info.ignore_count: # Reset ignore count after each stop
            bp.SetIgnoreCount(bp_info.ignore_count)

        # Evaluate condition if we have one
        try:
            if bp_info.condition and not bp_info.condition(bp_loc, frame, internal_dict):
                return False
        except Exception as e:
            self.console_err('Could not evaluate breakpoint condition: %s' % traceback.format_exc())
            return True

        # If we are supposed to stop and there's a log message, evaluate and print the message but don't stop.
        if  bp_info.log_message:
            try:
                def replacer(match):
                    expr = match.group(1)
                    result = self.evaluate_expr_in_frame(expr, frame)
                    result = expressions.Value.unwrap(result)
                    if isinstance(result, lldb.SBValue):
                        is_container = result.GetNumChildren() > 0
                        strvalue = self.get_var_value_str(result, self.global_format, is_container)
                    else:
                        strvalue = str(result)
                    return strvalue

                message = self.substitution_regex.sub(replacer, bp_info.log_message)
                self.console_msg(message)
                return False
            except Exception:
                self.console_err('Could not evaluate breakpoint log message: %s' % traceback.format_exc())
                return True

        return True

    def DEBUG_setExceptionBreakpoints(self, args):
        if self.launch_args.get('noDebug', False):
            return
        try:
            self.disable_bp_events()
            filters = args['filters']
            # Remove current exception breakpoints
            exc_bps = [bp_info.id for bp_info in self.breakpoints.values() if bp_info.kind == EXCEPTION]
            for bp_id in exc_bps:
                self.target.BreakpointDelete(bp_id)
                del self.breakpoints[bp_id]

            for bp in self.set_exception_breakpoints(filters):
                bp_info = BreakpointInfo(bp.GetID(), EXCEPTION)
                self.breakpoints[bp_info.id] = bp_info
        finally:
            self.enable_bp_events()

    def get_exception_filters(self, source_langs):
        default_panic = settings.get('defaultPanicBreakpoints', True)
        default_catch = settings.get('defaultCatchBreakpoints', False)

        filters = []
        if 'cpp' in source_langs:
            filters.extend([
                ('cpp_throw', 'C++: on throw', default_panic),
                ('cpp_catch', 'C++: on catch', default_catch),
            ])
        if 'rust' in source_langs:
            filters.extend([
                ('rust_panic', 'Rust: on panic', default_panic)
            ])
        return filters

    def set_exception_breakpoints(self, filters):
        cpp_throw = 'cpp_throw' in filters
        cpp_catch = 'cpp_catch' in filters
        rust_panic = 'rust_panic' in filters
        bps = []
        if cpp_throw or cpp_catch:
            bps.append(self.target.BreakpointCreateForException(lldb.eLanguageTypeC_plus_plus, cpp_catch, cpp_throw))
        if rust_panic:
            bps.append(self.target.BreakpointCreateByName('rust_panic'))
        return bps

    def DEBUG_configurationDone(self, args):
        try:
            self.pre_launch()
            result = self.do_launch(self.launch_args)
            # do_launch is asynchronous so we need to send its result
            self.send_response(self.launch_args['response'], result)
            # do this after launch, so that the debuggee does not inherit debugger's limits
            mem_limit.enable()
        except Exception as e:
            self.send_response(self.launch_args['response'], e)
        # Make sure VSCode knows if the process was initially stopped.
        if self.process is not None and self.process.is_stopped:
            self.update_threads()
            tid = next(iter(self.known_threads))
            self.send_event('stopped',  { 'allThreadsStopped': True, 'threadId': tid, 'reason': 'initial' })

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
                self.console_err(result.GetError())
                return

    def DEBUG_threads(self, args):
        threads = []
        for thread in self.process:
            index = thread.GetIndexID()
            tid = thread.GetThreadID()
            display = '%d: tid=%d' % (index, tid)
            name = thread.GetName()
            if name is not None:
                display += ' "%s"' % name
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
                fs = le.GetFileSpec()
                local_path = self.map_filespec_to_local(fs)
                if local_path is not None:
                    stack_frame['source'] = {
                        'name': fs.GetFilename(),
                        'path': local_path,
                        'origin': frame.GetModule().GetFileSpec().GetFilename()
                    }
                    stack_frame['line'] = le.GetLine()
                    stack_frame['column'] = le.GetColumn()
            else:
                pc_addr = frame.GetPCAddress()
                dasm = self.disassembly.get_by_address(pc_addr)
                if not dasm:
                    dasm = self.disassembly.create_from_address(pc_addr)
                if dasm:
                    stack_frame['source'] = {
                        'name': dasm.source_name,
                        'sourceReference': dasm.source_ref,
                        'origin': frame.GetModule().GetFileSpec().GetFilename()
                    }
                    stack_frame['line'] = dasm.line_num_by_address(pc_addr.GetLoadAddress(self.target))
                    stack_frame['column'] = 0

            if not frame.GetLineEntry().IsValid():
                stack_frame['presentationHint'] = 'subtle' # No line debug info.

            stack_frames.append(stack_frame)
        return { 'stackFrames': stack_frames, 'totalFrames': len(thread) }

    # Should we show source or disassembly for this frame?
    def in_disassembly(self, frame):
        if self.show_disassembly == 'never':
            return False
        elif self.show_disassembly == 'always':
            return True
        else:
            fs = frame.GetLineEntry().GetFileSpec()
            return self.map_filespec_to_local(fs) is None

    def DEBUG_source(self, args):
        sourceRef = int(args['sourceReference'])
        dasm = self.disassembly.get_by_handle(sourceRef)
        if not dasm:
            raise UserError('Source is not available.')
        return { 'content': dasm.get_source_text(), 'mimeType': 'text/x-lldb.disassembly' }

    def DEBUG_scopes(self, args):
        frame_id = args['frameId']
        frame = self.var_refs.get(frame_id)
        if frame is None:
            log.error('Invalid frame reference: %d', frame_id)
            return

        locals_scope_handle = self.var_refs.create(LocalsScope(frame), '[locs]', frame_id)
        locals = { 'name': 'Local', 'variablesReference': locals_scope_handle, 'expensive': False }
        statics_scope_handle = self.var_refs.create(StaticsScope(frame), '[stat]', frame_id)
        statics = { 'name': 'Static', 'variablesReference': statics_scope_handle, 'expensive': False }
        globals_scope_handle = self.var_refs.create(GlobalsScope(frame), '[glob]', frame_id)
        globals = { 'name': 'Global', 'variablesReference': globals_scope_handle, 'expensive': False }
        regs_scope_handle = self.var_refs.create(RegistersScope(frame), '[regs]', frame_id)
        registers = { 'name': 'Registers', 'variablesReference': regs_scope_handle, 'expensive': False }
        return { 'scopes': [locals, statics, globals, registers] }

    def DEBUG_variables(self, args):
        container_handle = args['variablesReference']
        container_info = self.var_refs.get_vpath(container_handle)
        if container_info is None:
            log.error('Invalid variables reference: %d', container_handle)
            return

        container, container_vpath = container_info
        container_name = None
        descendant_of_raw = False
        variables = collections.OrderedDict()
        if isinstance(container, LocalsScope):
            # args, locals, statics, in_scope_only
            vars_iter = SBValueListIter(container.frame.GetVariables(True, True, False, True))
            # Check if we have a return value from the last called function (usually after StepOut).
            ret_val = container.frame.GetThread().GetStopReturnValue()
            if ret_val.IsValid():
                name = '[return value]'
                dtype = ret_val.GetTypeName()
                handle = self.get_var_handle(ret_val, name, container_handle)
                value = self.get_var_value_str(ret_val, self.global_format, handle != 0)
                variable = {
                    'name': name,
                    'value': value,
                    'type': dtype,
                    'variablesReference': handle
                }
                variables[name] = variable
        elif isinstance(container, StaticsScope):
            vars_iter = (v for v in SBValueListIter(container.frame.GetVariables(False, False, True, True))
                         if v.GetValueType() == lldb.eValueTypeVariableStatic)
        elif isinstance(container, GlobalsScope):
            vars_iter = (v for v in SBValueListIter(container.frame.GetVariables(False, False, True, True))
                         if v.GetValueType() != lldb.eValueTypeVariableStatic)
        elif isinstance(container, RegistersScope):
            vars_iter = SBValueListIter(container.frame.GetRegisters())
        elif isinstance(container, lldb.SBValue):
            value_type = container.GetValueType()
            if value_type != lldb.eValueTypeRegisterSet: # Registers are addressed by name, without parent reference.
                # First element in vpath is the stack frame, second - the scope object.
                for segment in container_vpath[2:]:
                    container_name = compose_eval_name(container_name, segment)
            vars_iter = SBValueChildrenIter(container)
            # PreferSyntheticValue is a sticky flag passed on to child values;
            # we use it to identify descendents of the [raw] node, since that's the only time we reset it.
            descendant_of_raw = not container.GetPreferSyntheticValue()

        time_limit = time.clock() + self.evaluation_timeout
        for var in vars_iter:
            if not var.IsValid():
                continue
            name = var.GetName()
            if name is None:
                name = ''
            dtype = var.GetTypeName()
            handle = self.get_var_handle(var, name, container_handle)
            value = self.get_var_value_str(var, self.global_format, handle != 0)

            if not descendant_of_raw:
                evalName = compose_eval_name(container_name, name)
            else:
                stm = lldb.SBStream()
                var.GetExpressionPath(stm)
                evalName = '/nat ' + stm.GetData()

            variable = {
                'name': name,
                'value': value,
                'type': dtype,
                'variablesReference': handle,
                'evaluateName': evalName
            }
            # Ensure proper variable shadowing: if variable of the same name had already been added,
            # remove it and insert the new instance at the end.
            if name in variables:
                del variables[name]
            variables[name] = variable

            if time.clock() > time_limit:
                self.console_err('Child list expansion has timed out.')
                break

        variables = list(variables.values())

        # If this node was synthetic (i.e. a product of a visualizer),
        # append [raw] pseudo-child, which can be expanded to show raw view.
        if isinstance(container, lldb.SBValue) and container.IsSynthetic():
            raw_var = container.GetNonSyntheticValue()
            stm = lldb.SBStream()
            raw_var.GetExpressionPath(stm)
            evalName = '/nat ' + stm.GetData()
            handle = self.var_refs.create(raw_var, '[raw]', container_handle)
            variable = {
                'name': '[raw]',
                'value': container.GetTypeName(),
                'variablesReference': handle,
                'evaluateName': evalName
            }
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
        expr = args['expression']
        if context in ['watch', 'hover', None]: # 'Copy Value' in Locals does not send context.
            return self.evaluate_expr(args, expr)
        elif expr.startswith('?'): # "?<expr>" in 'repl' context
            return self.evaluate_expr(args, expr[1:])
        # Else evaluate as debugger command
        frame = self.var_refs.get(args.get('frameId'), None)
        result = self.execute_command_in_frame(expr, frame)
        output = result.GetOutput() if result.Succeeded() else result.GetError()
        return { 'result': from_lldb_str(output or '') }

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

    # Classify expression by evaluator type
    def get_expression_type(self, expr):
        if expr.startswith('/nat '):
            return NATIVE, expr[5:]
        elif expr.startswith('/py '):
            return PYTHON, expr[4:]
        elif expr.startswith('/se '):
            return SIMPLE, expr[4:]
        else:
            return self.launch_args.get('expressions', SIMPLE), expr

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

        if isinstance(result, lldb.SBError): # Evaluation error
            error_message = result.GetCString()
            if context == 'repl':
                self.console_err(error_message)
                return None
            else:
                raise UserError(error_message.replace('\n', '; '), no_console=True)

        # Success
        result = expressions.Value.unwrap(result)
        if isinstance(result, lldb.SBValue):
            dtype = result.GetTypeName();
            handle = self.get_var_handle(result, expr, None)
            value = self.get_var_value_str(result, format, handle != 0)
            return { 'result': value, 'type': dtype, 'variablesReference': handle }
        else: # Some Python value
            return { 'result': str(result), 'variablesReference': 0 }

    # Evaluating these names causes LLDB exit, not sure why.
    pyeval_globals = { 'exit':None, 'quit':None, 'globals':None }

    # Evaluates expr in the context of frame (or in global context if frame is None)
    # Returns expressions.Value or SBValue on success, SBError on failure.
    def evaluate_expr_in_frame(self, expr, frame):
        ty, expr = self.get_expression_type(expr)
        if ty == NATIVE:
            if frame is not None:
                result = frame.EvaluateExpression(to_lldb_str(expr)) # In frame context
            else:
                result = self.target.EvaluateExpression(to_lldb_str(expr)) # In global context
            error = result.GetError()
            if error.Success():
                return result
            else:
                return error
        else:
            if frame is None: # Use the currently selected frame
                frame = self.process.GetSelectedThread().GetSelectedFrame()

            if ty == PYTHON:
                expr = expressions.preprocess_python_expr(expr)
                self.set_selected_frame(frame)
                eval_globals = self.session_dict
                eval_globals['__frame_vars'] = expressions.PyEvalContext(frame)
                eval_locals = {}
            else: # SIMPLE
                expr = expressions.preprocess_simple_expr(expr)
                log.info('Preprocessed expr: %s', expr)
                eval_globals = self.pyeval_globals
                eval_locals = expressions.PyEvalContext(frame)
                eval_globals['__frame_vars'] = eval_locals

            try:
                log.info('Evaluating %s', expr)
                return eval(expr, eval_globals, eval_locals)
            except Exception as e:
                log.info('Evaluation error: %s', traceback.format_exc())
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

    # Extracts a printable value from SBValue.
    def get_var_value_str(self, var, format, is_container):
        expressions.analyze(var)
        var.SetFormat(format)
        value = None

        if self.deref_pointers and format == lldb.eFormatDefault:
            ptr_type = var.GetType()
            if ptr_type.GetTypeClass() in [lldb.eTypeClassPointer, lldb.eTypeClassReference]:
                # If pointer has associated synthetic, or if it's a pointer to basic type such as `char`,
                # use summary of the pointer itself,
                # otherwise prefer to dereference and use summary of the pointee.
                if var.IsSynthetic() or ptr_type.GetPointeeType().GetBasicType() != 0:
                    value = var.GetSummary()

                if value is None:
                    # check whether it's an invalid pointer
                    addr = var.GetValueAsUnsigned()
                    if addr == 0:
                        value = '<null>'
                    else:
                        error = lldb.SBError()
                        self.process.ReadMemory(addr, 1, error)
                        if error.Fail():
                            value = '<invalid address>' # invalid address other than NULL
                        else:
                            var = var.Dereference()

        if value is None:
            value = var.GetValue()
            if value is None:
                value = var.GetSummary()

        if value is None:
            if is_container:
                if self.container_summary:
                    value =  self.get_container_summary(var, format)
                else:
                    value = '{...}'
            else:
                value = '<not available>'

        # deal with encodings
        if value is not None:
            value = value.replace('\n', '') # VSCode won't display line breaks
            value = from_lldb_str(value)

        return value


    def get_container_summary(self, var, format, maxsize=32):
        summary = ['{']
        size = 0
        n = var.GetNumChildren()
        for i in xrange(0, n):
            child = var.GetChildAtIndex(i)
            name = child.GetName() or ''
            value = child.GetValue()
            if value is not None:
                if size > 0:
                    summary.append(', ')

                if self.ordinal_name.match(name):
                    summary.append(value)
                    size += len(value)
                else:
                    summary.append(name)
                    summary.append(':')
                    summary.append(value)
                    size += len(name) + len(value) + 1

                if size > maxsize:
                    summary.append(', ...}')
                    break
        else:
            if size == 0:
                return '{...}'
            else:
                summary.append('}')
        return ''.join(summary)

    ordinal_name = re.compile(r'\[\d+\]')

    # Generate a handle for a variable.
    def get_var_handle(self, var, key, parent_handle):
        if var.GetNumChildren() > 0 or var.IsSynthetic(): # Might have children
            return self.var_refs.create(var, key, parent_handle)
        else:
            return 0

    # Clears out cached state that become invalid once debuggee resumes.
    def before_resume(self):
        self.var_refs.reset()

    def DEBUG_setVariable(self, args):
        container = self.var_refs.get(args['variablesReference'])
        if container is None:
            raise Exception('Invalid variables reference')

        name = to_lldb_str(args['name'])
        var = None
        if isinstance(container, (LocalsScope, StaticsScope, GlobalsScope)):
            # args, locals, statics, in_scope_only
            var = expressions.find_var_in_frame(container.frame, name)
        elif isinstance(container, lldb.SBValue):
            var = container.GetChildMemberWithName(name)
            if not var.IsValid():
                var = container.GetValueForExpressionPath(name)

        if var is None or not var.IsValid():
            raise UserError('Could not set variable %r %r\n' % (name, var.IsValid()))

        error = lldb.SBError()
        if not var.SetValueFromCString(to_lldb_str(args['value']), error):
            raise UserError(error.GetCString())
        return { 'value': self.get_var_value_str(var, self.global_format, False) }

    def DEBUG_disconnect(self, args):
        if self.launch_args is not None:
            self.exec_commands(self.launch_args.get('exitCommands'))
        if self.process:
            self.process.GetBroadcaster().RemoveListener(self.event_listener)
            if args.get('terminateDebuggee', self.process_launched):
                self.process.Kill()
            else:
                self.process.Detach()
        self.restart = args.get('restart', False)
        self.process = None
        self.target = None
        self.terminal = None
        self.listener_handler_token = None
        self.event_loop.stop()

    def DEBUG_test(self, args):
        self.console_msg('TEST')

    def DEBUG_adapterSettings(self, args):
        self.update_display_settings(args)
        self.refresh_client_display()

    def update_display_settings(self, settings):
        if not settings: return

        self.show_disassembly = settings.get('showDisassembly', 'auto')

        format = settings.get('displayFormat', 'auto')
        if format == 'hex':
            self.global_format = lldb.eFormatHex
        elif format == 'decimal':
            self.global_format = lldb.eFormatDecimal
        elif format == 'binary':
            self.global_format = lldb.eFormatBinary
        else:
            self.global_format = lldb.eFormatDefault

        self.deref_pointers = settings.get('dereferencePointers', True)

        self.container_summary = settings.get('containerSummary', True)

        self.console_msg('Display settings: variable format=%s, show disassembly=%s, numeric pointer values=%s, container summaries=%s.' % (
                         format, self.show_disassembly,
                         'off' if self.deref_pointers else 'on',
                         'on' if self.container_summary else 'off'))

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
            self.DEBUG_disconnect({})
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

            log.info('### Handling command: %s', command)
            if log.isEnabledFor(logging.DEBUG):
                log.debug('Command args: %s', json.dumps(args, indent=4))
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
            if log.isEnabledFor(logging.DEBUG):
                log.debug('Command result: %s', json.dumps(result, indent=4))
            response['success'] = True
            response['body'] = result
        elif isinstance(result, UserError):
            log.debug('Command result is UserError: %s', result)
            if not result.no_console:
                self.console_err('Error: ' + str(result))
            response['success'] = False
            response['body'] = { 'error': { 'id': 0, 'format': str(result), 'showUser': True } }
        elif isinstance(result, Exception):
            tb = traceback.format_exc()
            log.error('Internal debugger error:\n%s', tb)
            self.console_err('Internal debugger error:\n' + tb)
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
            self.notify_process(event)
        elif lldb.SBBreakpoint.EventIsBreakpointEvent(event):
            self.notify_breakpoint(event)
        elif lldb.SBTarget.EventIsTargetEvent(event):
            self.notify_target(event)

    # Handles process state change notifications
    def notify_process(self, event):
        ev_type = event.GetType()
        if ev_type == lldb.SBProcess.eBroadcastBitStateChanged:
            state = lldb.SBProcess.GetStateFromEvent(event)
            if state == lldb.eStateRunning:
                self.send_event('continued', { 'threadId': 0, 'allThreadsContinued': True })
            elif state == lldb.eStateStopped:
                if not lldb.SBProcess.GetRestartedFromEvent(event):
                    self.notify_target_stopped(event)
            elif state == lldb.eStateCrashed:
                self.notify_target_stopped(event)
            elif state == lldb.eStateExited:
                exit_code = self.process.GetExitStatus()
                self.console_msg('Process exited with code %d.' % exit_code)
                self.send_event('exited', { 'exitCode': exit_code })
                self.send_event('terminated', {}) # TODO: VSCode doesn't seem to handle 'exited' for now
            elif state == lldb.eStateDetached:
                self.console_msg('Debugger has detached from process.')
                self.send_event('terminated', {})
        elif ev_type & (lldb.SBProcess.eBroadcastBitSTDOUT | lldb.SBProcess.eBroadcastBitSTDERR) != 0:
            self.notify_stdio(ev_type)

    def notify_target_stopped(self, lldb_event):
        self.update_threads()
        event = { 'allThreadsStopped': True } # LLDB always stops all threads
        # Find the thread that caused this stop
        stopped_thread = None

        # Check the currently selected thread first
        selected_thread = self.process.GetSelectedThread()
        if selected_thread is not None and selected_thread.IsValid():
            stop_reason = selected_thread.GetStopReason()
            if stop_reason != lldb.eStopReasonInvalid and stop_reason != lldb.eStopReasonNone:
                stopped_thread = selected_thread

        # Fall back to scanning all threads in the process
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
                stop_reason_str = 'breakpoint'
                if stopped_thread.GetStopReasonDataCount() >= 2:
                    bp_id = stopped_thread.GetStopReasonDataAtIndex(0)
                    bploc_id = stopped_thread.GetStopReasonDataAtIndex(1)
                    bp_info = self.breakpoints.get(bp_id)
                    if bp_info:
                        if bp_info.kind == EXCEPTION:
                            stop_reason_str = 'exception'
                        elif bp_info.kind == SOURCE:
                            bp = self.target.FindBreakpointByID(bp_id)
                            bp_loc = bp.FindLocationByID(bploc_id)
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

    # Handles breakpoint change notifications.
    def notify_breakpoint(self, event):
        event_type = lldb.SBBreakpoint.GetBreakpointEventTypeFromEvent(event)
        bp = lldb.SBBreakpoint.GetBreakpointFromEvent(event)
        if event_type == lldb.eBreakpointEventTypeAdded:
            self.notify_breakpoint_added(bp, event)
        elif event_type == lldb.eBreakpointEventTypeLocationsResolved:
            self.notify_breakpoint_resolved(bp, event)
        elif event_type == lldb.eBreakpointEventTypeRemoved:
            bp_id = bp.GetID()
            self.send_event('breakpoint', { 'reason': 'removed', 'breakpoint': { 'id': bp_id } })
            del self.breakpoints[bp_id]

    def notify_breakpoint_added(self, bp, event):
        bp_id = bp.GetID()
        loc = bp.GetLocationAtIndex(0)
        addr = loc.GetAddress()
        le = addr.GetLineEntry()
        if le:
            bp_info = BreakpointInfo(bp_id, SOURCE)
            bp_info.file_path = le.GetFileSpec().fullpath
            bp_info.line = le.GetLine()
        else:
            bp_info = BreakpointInfo(bp_id, ASSEMBLY)
            bp_info.address = addr.GetLoadAddress(self.target)
        self.breakpoints[bp_id] = bp_info
        bp_resp = self.make_bp_resp(bp, bp_info)
        self.send_event('breakpoint', { 'reason': 'new', 'breakpoint': bp_resp })

    def notify_breakpoint_resolved(self, bp, event):
        bp_id = bp.GetID()
        bp_info = self.breakpoints.get(bp_id)
        if bp_info is None:
            return
        if bp_info.kind == SOURCE:
            num_locs = lldb.SBBreakpoint.GetNumBreakpointLocationsFromEvent(event)
            bp_locs = [lldb.SBBreakpoint.GetBreakpointLocationAtIndexFromEvent(event, i) for i in range(num_locs)]
            for bp_loc in bp_locs:
                if bp_loc.IsResolved():
                    bp_info.verified = True
        breakpoint = self.make_bp_resp(bp, bp_info)
        self.send_event('breakpoint', { 'reason': 'changed', 'breakpoint': breakpoint })

    def notify_target(self, event):
        if event.GetType() & lldb.SBTarget.eBroadcastBitModulesLoaded != 0:
            for i in xrange(lldb.SBTarget.GetNumModulesFromEvent(event)):
                mod = lldb.SBTarget.GetModuleAtIndexFromEvent(i, event)
                message = 'Module loaded: %s.' % mod.GetFileSpec().fullpath
                if mod.GetSymbolFileSpec().IsValid():
                    message += ' Symbols loaded.'
                self.console_msg(message)

    def handle_debugger_output(self, output):
        self.send_event('output', { 'category': 'stdout', 'output': output })

    def send_event(self, event, body):
        message = {
            'type': 'event',
            'seq': 0,
            'event': event,
            'body': body
        }
        self.send_message(message)

    # Write a message to debug console
    def console_msg(self, output, category=None):
        if output:
            self.send_event('output', {
                'category': category,
                'output': from_lldb_str(output) + '\n'
            })

    def console_err(self, output):
        self.console_msg(output, 'stderr')

    # Translates SBFileSpec into a local path using mappings in source_map.
    # Returns None if source info should be suppressed.  There are 3 cases when this happens:
    # - filespec.IsValid() is false,
    # - user has directed us to suppress source info by setting the local prefix is source map to None,
    # - suppress_missing_sources is true and the local file does not exist.
    def map_filespec_to_local(self, filespec):
        if not filespec.IsValid():
            return None
        local_path = os.path.normpath(filespec.fullpath)
        if self.suppress_missing_sources and not os.path.isfile(local_path):
            local_path = None
        return local_path

    # Ask VSCode extension to display HTML content.
    def display_html(self, body):
        self.send_event('displayHtml', body)

def on_breakpoint_hit(frame, bp_loc, internal_dict):
    return DebugSession.current.should_stop_on_bp(bp_loc, frame, internal_dict)

# For when we need to let user know they screwed up
class UserError(Exception):
    def __init__(self, message, no_console=False):
        Exception.__init__(self, message)
        # Don't copy error message to debug console if this is set
        self.no_console = no_console

# Result type for async handlers
class AsyncResponse:
    pass

class LocalsScope:
    def __init__(self, frame):
        self.frame = frame

class StaticsScope:
    def __init__(self, frame):
        self.frame = frame

class GlobalsScope:
    def __init__(self, frame):
        self.frame = frame

class RegistersScope:
    def __init__(self, frame):
        self.frame = frame

# Various info we mantain about a breakpoint
class BreakpointInfo:
    __slots__ = ['id', 'kind', 'condition', 'ignore_count', 'log_message',
                 'address', 'adapter_data',
                 'file_path', 'line', 'verified']
    def __init__(self, id, kind):
        self.id = id
        self.kind = kind          # SOURCE | FUNCTION | ASSEMBLY | EXCEPTION
        self.condition = None
        self.log_message = None
        self.ignore_count = 0
        # ASSEMBLY only
        self.address = None       # Breakpoint address.
        self.adapter_data = None  # Data needed to reconstruct disassembly source across sessions.
        # SOURCE only
        self.file_path = None     # Source file.
        self.line = None          # Source line
        self.verified = False     # Is it resolved

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

def compose_eval_name(container, var_name):
    if container is None:
        return expressions.escape_variable_name(var_name)
    elif var_name.startswith('['):
        return container + var_name
    else:
        return container + '.' + expressions.escape_variable_name(var_name)

