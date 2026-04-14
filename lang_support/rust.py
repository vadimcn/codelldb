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
    debugger.HandleCommand("type summary add --category Rust --python-function lang_support.rust.char_summary 'char32_t'")

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
            try:
                sysroot = subprocess.check_output(command, startupinfo=si, encoding='utf-8').strip()
            except (OSError, subprocess.CalledProcessError) as err:
                log.exception(f'Could not execute {command}')
                codelldb.debugger_message('Could not locate Rust toolchain', category='stderr')
                return
        formatters = path.join(sysroot, 'lib/rustlib/etc')

    log.info(f'Rust formatters dir: {formatters}')
    lldb_lookup = path.join(formatters, 'lldb_lookup.py')
    lldb_commands = path.join(formatters, 'lldb_commands')
    if not path.isfile(lldb_lookup) or not path.isfile(lldb_commands):
        message = 'Could not find LLDB data formatters in your Rust toolchain.'
        if sysroot and '-msvc' in sysroot:
            message += '  For more information, please visit https://github.com/vadimcn/codelldb/wiki/Windows'
        codelldb.debugger_message(message, category='stderr')
        return

    codelldb.debugger_message('Loading Rust formatters from {}'.format(formatters))
    debugger.HandleCommand("command script import '{}'".format(lldb_lookup))
    with open(lldb_commands, 'rt') as f:
        for line in f:
            line = line.strip()
            # On LLDB versions that support recognizer functions, detect this line:
            #   "type synthetic add -l lldb_lookup.synthetic_lookup -x ".*" --category Rust"
            # and add a recognizer function to skip obvious non-Rust types, such as integers and floats.
            if (version_major >= 19 and
                    line.startswith('type synthetic add') and '-x ".*"' in line):
                line = line.replace('-x ".*"', '--recognizer-function lang_support.rust.is_rust_type')
            if line and not line.startswith('#'):
                debugger.HandleCommand(line)


def is_rust_type(sbtype, internal_dict):
    return sbtype.GetTypeClass() != lldb.eTypeClassBuiltin


def char_summary(valobj, internal_dict):
    v = valobj.GetValueAsUnsigned()
    if v > 0x10FFFF or (0xD800 <= v <= 0xDFFF):
        return f"U+{v:04X} (invalid)"
    ch = chr(v)
    if not ch.isprintable():
        return f"U+{v:04X}"
    return f"U+{v:04X} '{ch}'"
