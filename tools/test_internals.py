#!/usr/bin/python
# Execute tests in Python code
import set_lldb_path
from adapter import expressions
expressions.run_tests()
print('Success')
