from .api import evaluate, wrap, unwrap, get_config, create_webview, display_html, debugger_message
from .value import Value

def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    import logging
    logging.basicConfig(level=logging.DEBUG,  # filename='/tmp/codelldb.log',
                        format='%(levelname)s(Python) %(asctime)s %(name)s: %(message)s', datefmt='%H:%M:%S')
