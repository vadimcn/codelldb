import lldb
import logging
from os import path
import subprocess
import codelldb

log = logging.getLogger(__name__)


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    lldb.SBDebugger.SetInternalVariable('target.process.thread.step-avoid-regexp',
                                        '^<?(std|core|alloc)::', debugger.GetInstanceName())

    debugger.HandleCommand("type format add --category Rust --format d 'char' 'signed char'")
    debugger.HandleCommand("type format add --category Rust --format u 'unsigned char'")

    sysroot = codelldb.get_config('lang.rust.sysroot')
    if sysroot is None:
        toolchain = codelldb.get_config('lang.rust.toolchain')
        if toolchain is not None:
            command = ['rustup', 'run', toolchain, 'rustc', '--print=sysroot']
        else:
            command = ['rustc', '--print=sysroot']

        si = None
        if hasattr(subprocess, 'STARTUPINFO'):
            si = subprocess.STARTUPINFO(dwFlags=subprocess.STARTF_USESHOWWINDOW,  # type: ignore
                                        wShowWindow=subprocess.SW_HIDE)  # type: ignore
        sysroot = subprocess.check_output(command, startupinfo=si, encoding='utf-8').strip()
    etc = path.join(sysroot, 'lib/rustlib/etc')

    codelldb.debugger_message('Loading Rust formatters from {}'.format(etc))
    lldb_lookup = path.join(etc, 'lldb_lookup.py')
    lldb_commands = path.join(etc, 'lldb_commands')
    if path.isfile(lldb_lookup):
        debugger.HandleCommand("command script import '{}'".format(lldb_lookup))
        debugger.HandleCommand("command source -s true '{}'".format(lldb_commands))
    else:
        if '-msvc' in sysroot:
            codelldb.debugger_message(
                'Could not find LLDB data formatters in your Rust toolchain.  '
                'For more information, please visit https://github.com/vadimcn/codelldb/wiki/Windows',
                category='stderr')
