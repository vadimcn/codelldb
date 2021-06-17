import lldb
import re

def gdb_escape(string):
    result = ""
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

@lldb.command('rr')
def execute(debugger, command, result, internal_dict):
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
