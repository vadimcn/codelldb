import logging
from . import rust

log = logging.getLogger(__name__)

def __lldb_init_module(debugger_obj, internal_dict):
    log.info('Initializing')
    rust.__lldb_init_module(debugger_obj, internal_dict)
