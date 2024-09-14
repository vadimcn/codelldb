import lldb
import logging
from os import path

log = logging.getLogger(__name__)


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    for lang in internal_dict['source_languages']:
        try:
            ns = __import__('lang_support', fromlist=[lang])
        except ImportError:
            pass

        try:
            getattr(ns, lang).__lldb_init_module(debugger, internal_dict)
        except:
            log.exception('Failed to initialize language support for {}'.format(lang))
