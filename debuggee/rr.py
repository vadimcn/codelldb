from __future__ import print_function
import lldb
import re

def gdb_escape(string):
    result = ""
    pos = 0
    for curr_char in string:
        result += format(ord(curr_char), '02x')
    return result

def gdb_unescape(string):
    result = ""
    pos = 0
    while pos < len(string):
        result += chr(int(string[pos:pos+2], 16))
        pos += 2
    return result

def execute(debugger, command, result, internal_dict):
    #print 'process plugin packet send \'qRRCmd:%s\'' % command
    cmd = 'process plugin packet send \'qRRCmd:%s\'' % gdb_escape(command)
    interp = debugger.GetCommandInterpreter()
    interp.HandleCommand(cmd, result, False)
    if result.Succeeded():
        rv = result.GetOutput()
        rv_match = re.search('response: ([0-9a-fA-F]*)', rv, re.MULTILINE);
        rv = gdb_unescape(rv_match.group(1))
        result.Clear()
        result.PutCString(rv)
        result.SetStatus(lldb.eReturnStatusSuccessFinishResult)

def __lldb_init_module(debugger, internal_dict):
    debugger.HandleCommand('command script add -f rr.execute rr')
