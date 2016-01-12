import asyncore
import debugserver
import debugsession
from six import print_

PORT = 4711

print_("Starting server on port", PORT)
server = debugserver.DebugServer('localhost', PORT, debugsession.DebugSession)
asyncore.loop()
