#!/bin/sh
exec lldb -b -O "command script import $(dirname $0)/adapter" -O "script adapter.run_stdio_session()"
