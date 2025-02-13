import lldb
import argparse
import shlex


class DebugInfoCommand:
    def __init__(self, debugger, internal_dict):
        self.parser = argparse.ArgumentParser('debug_info')
        subparsers = self.parser.add_subparsers(dest='subcommand')
        list = subparsers.add_parser('list', help='List modules having source-level debug information.')
        list.add_argument('filter', metavar='<regex>', nargs="?", default=None, help='Module name filter')
        show = subparsers.add_parser('show', help='Show compilation units present in module\'s debug information.')
        show.add_argument('module', nargs="?", default=None, help='Module name')
        show.add_argument('--file', help='Module file path')

    @staticmethod
    def register(debugger):
        debugger.HandleCommand('command script add -c codelldb.debug_info.DebugInfoCommand debug_info')

    def __call__(self, debugger, command, exe_ctx, result):
        try:
            args = self.parser.parse_args(shlex.split(command))
            if args.subcommand == 'list':
                self.sub_list(args, debugger, exe_ctx, result)
            elif args.subcommand == 'show':
                self.sub_show(args, debugger, exe_ctx, result)
            else:
                self.parser.print_help(result)
        except SystemExit:
            self.parser.print_help(result)
        result.flush()

    def sub_list(self, args, debugger, exe_ctx, result):
        filter = self.get_mod_filter(args.filter)
        for module in exe_ctx.target.modules:
            if filter(module):
                result.write('Module {} : {} compile units with debug info\n'.format(
                    module.platform_file.fullpath, len(module.compile_units)))

    def sub_show(self, args, debugger, exe_ctx, result):
        def dump_module(module):
            for cu in module.compile_units:
                result.write('    {}\n'.format(cu.file.fullpath))
            result.write('\n')

        if args.file is not None:
            debugger = lldb.SBDebugger.Create()
            error = lldb.SBError()
            target = debugger.CreateTarget(args.file, None, None, False, error)
            dump_module(target.modules[0])
            lldb.SBDebugger.Destroy(debugger)
        else:
            filter = self.get_mod_filter(args.module)
            for module in exe_ctx.target.modules:
                if filter(module):
                    dump_module(module)
                    break
            else:
                result.write('Module "{}" not found.'.format(args.module))

    def get_mod_filter(self, filter):
        if filter:
            import re
            regex = re.compile(filter)
            return lambda mod: regex.search(mod.file.fullpath)
        else:
            return lambda mod: True
