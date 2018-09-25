#!/usr/bin/python
# Debug stub for launching Python debug session inside LLDB
import sys
import subprocess

args = ['*'] + sys.argv[3:]
script = [
  "import sys, runpy, __main__",
  "sys.orig_main = __main__",
  "sys.argv=['%s']" % "','".join(args),
  "runpy.run_module('ptvsd', alter_sys=True, run_name='__main__')"
]
command = ['lldb-6.0', '-b', '-O', 'script ' + '; '.join(script)]
subprocess.call(command)
