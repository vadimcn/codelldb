import sys
sys.modules['__main__'] = sys.orig_main

import adapter
adapter.run_tcp_server()
