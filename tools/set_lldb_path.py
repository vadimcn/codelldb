import sys
import os
import subprocess

lldb = os.environ.get('LLDB_EXECUTABLE', 'lldb')

out = subprocess.check_output([lldb, '-P'])
sys.path.append(out.decode('utf8').strip())
sys.path.append('.')
