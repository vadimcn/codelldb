#!/usr/bin/python
import sys
import subprocess

out = subprocess.check_output(['lldb', '-P'])
sys.path.append(out.strip())
sys.path.append('.')

import adapter
adapter.main.run_tcp_server()
