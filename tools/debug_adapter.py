#!/usr/bin/python
import sys
import subprocess
import string

out = subprocess.check_output(['lldb', '-P'])
sys.path.append(string.strip(out))
sys.path.append('.')

import adapter
adapter.main.run_tcp_server()
