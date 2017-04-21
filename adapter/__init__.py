from __future__ import print_function

def escape(message):
    return message.replace('\n', '\\n').replace('"', '\\"')

def show_message(message):
    json = ('{"type":"response","command":"initialize","request_seq":1,"success":false,'+
                '"body":{"error":{"id":0,"format":"%s","showUser":true}}}') % escape(message)
    print('\r\nContent-Length: %d\r\n\r\n%s' % (len(json), json))

def log_to_console(message):
    json = '{"type":"event","seq":0,"event":"output","body":{"category":"console","output":"%s"}}' % escape(message)
    print('\r\nContent-Length: %d\r\n\r\n%s' % (len(json), json))

try:
    stage = 0
    import sys

    # Test for Brew and MacPorts brokenness.
    stage = 1
    import io

    stage = 2
    PY2 = sys.version_info[0] == 2
    if PY2:
        is_string = lambda v: isinstance(v, basestring)
        xrange = xrange
    else:
        is_string = lambda v: isinstance(v, str)
        xrange = range
    import adapter.main

except Exception as e:
    show_message('The debug adapter has encountered an error during startup.  Please check Debug Console for details.')
    if stage == 1 and 'darwin' in sys.platform:
        log_to_console('*** This error is likely caused by a conflict with Brew or MacPorts installed Python.\n'+
                       '*** Please see the Troubleshooting page on Wiki.\n\n')
    import traceback
    tb = traceback.format_exc()
    log_to_console(tb)
