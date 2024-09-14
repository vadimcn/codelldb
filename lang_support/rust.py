import lldb
import logging
from os import path
import subprocess

log = logging.getLogger(__name__)


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    lldb.SBDebugger.SetInternalVariable('target.process.thread.step-avoid-regexp',
                                        '^<?(std|core|alloc)::', debugger.GetInstanceName())

    sysroot = subprocess.check_output(
        ['rustc', '--print=sysroot'], encoding='utf-8').strip()
    etc = path.join(sysroot, 'lib/rustlib/etc')
    log.info('Loading Rust formatters from {}'.format(etc))
    debugger.HandleCommand("command script import '{}'".format(
        path.join(etc, 'lldb_lookup.py')))
    debugger.HandleCommand(
        "command source -s true '{}'".format(path.join(etc, 'lldb_commands')))

    import lldb_lookup
    if hasattr(lldb_lookup, 'ClangEncodedEnumProvider'):
        class EnumWrapper(lldb_lookup.ClangEncodedEnumProvider):
            def __init__(self, valobj, dict):
                super().__init__(valobj, dict)

            def update(self):
                super().update()
                self.value = super().get_child_at_index(0)

            def num_children(self):
                return self.value.GetNumChildren()

            def get_child_index(self, name):
                return self.value.GetIndexOfChildWithName(name)

            def get_child_at_index(self, index):
                return self.value.GetChildAtIndex(index)

            def has_children(self):
                return self.value.MightHaveChildren()

            def get_value(self):
                return self.value.GetValue()

            def get_type_name(self):
                return self.value.GetTypeName()

        lldb_lookup.ClangEncodedEnumProvider = EnumWrapper
    else:
        log.error('lldb_lookup.ClangEncodedEnumProvider not found')
