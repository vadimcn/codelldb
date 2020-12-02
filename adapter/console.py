import sys
import lldb

@lldb.command('pip')
def pip(debugger, command, result, internal_dict):
    import runpy
    import shlex
    org_argv = sys.argv.copy()
    sys.argv[1:] = shlex.split(command)
    try:
        runpy.run_module('pip', run_name='__main__', alter_sys=True)
    finally:
        sys.argv = org_argv

print()
print('Extra commands available:')
print('    pip - Manage Python local site packages.')
print()
