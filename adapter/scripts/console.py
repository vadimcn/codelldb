import sys
from codelldb import commands


def pip(debugger, command, result, internal_dict):
    import runpy
    import shlex
    org_argv = sys.argv.copy()
    sys.argv[1:] = shlex.split(command)
    try:
        runpy.run_module('pip', run_name='__main__', alter_sys=True)
    finally:
        sys.argv = org_argv


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    debugger.HandleCommand('command script add -f console.pip pip')
    commands.register(debugger)
    print()
    print('Extra commands available:')
    print('    pip        - Manage Python packages.')
    print('    debug_info - Show module debug information.')
    print()
