import lldb
import logging
from os import path
import subprocess
import codelldb

log = logging.getLogger(__name__)


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    lldb.SBDebugger.SetInternalVariable('target.process.thread.step-avoid-regexp',
                                        '^<?(std|core|alloc)::', debugger.GetInstanceName())

    debugger.HandleCommand("type format add --category Rust --format dec 'char' 'unsigned char'")

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
    log.info('Loading Rust formatters from {}'.format(etc))
    debugger.HandleCommand("command script import '{}'".format(path.join(etc, 'lldb_lookup.py')))
    debugger.HandleCommand("command source -s true '{}'".format(path.join(etc, 'lldb_commands')))
