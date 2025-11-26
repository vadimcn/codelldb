import lldb
import logging
from os import path
import subprocess
import codelldb

log = logging.getLogger(__name__)


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    try:
        version = lldb.SBDebugger.GetVersionString()
        version_major = int(version[version.find('version ') + 8:].split('.')[0])
    except Exception:
        version_major = 0

    lldb.SBDebugger.SetInternalVariable('target.process.thread.step-avoid-regexp',
                                        '^<?(std|core|alloc)::', debugger.GetInstanceName())

    debugger.HandleCommand("type format add --category Rust --format d 'char' 'signed char'")
    debugger.HandleCommand("type format add --category Rust --format u 'unsigned char'")

    sysroot = None
    formatters = codelldb.get_config('lang.rust.formatters')
    if formatters is None:
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
        formatters = path.join(sysroot, 'lib/rustlib/etc')

    codelldb.debugger_message('Loading Rust formatters from {}'.format(formatters))
    lldb_lookup = path.join(formatters, 'lldb_lookup.py')
    lldb_commands = path.join(formatters, 'lldb_commands')
    if path.isfile(lldb_lookup):
        debugger.HandleCommand("command script import '{}'".format(lldb_lookup))
        use_recognizer_fn = version_major >= 19 and hasattr(internal_dict['lldb_lookup'], 'classify_rust_type')
        with open(lldb_commands, 'rt') as f:
            for line in f:
                if use_recognizer_fn and line.startswith('type synthetic') and '-x ".*"' in line:
                    # Replace wildcard matching with a recognizer function so Rust synthetics do not get attached
                    # to types we do not intend to handle, such as ints or floats.
                    line = 'type synthetic add -l lldb_lookup.synthetic_lookup --recognizer-function lang_support.rust.is_rust_type --category Rust'
                debugger.HandleCommand(line.strip())
    else:
        if sysroot and '-msvc' in sysroot:
            codelldb.debugger_message(
                'Could not find LLDB data formatters in your Rust toolchain.  '
                'For more information, please visit https://github.com/vadimcn/codelldb/wiki/Windows',
                category='stderr')


def is_rust_type(sbtype, internal_dict):
    kind = internal_dict['lldb_lookup'].classify_rust_type(sbtype)
    return kind != 'Other'
