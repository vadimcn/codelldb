import logging
from os import path

log = logging.getLogger(__name__)


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    adapter_settings = internal_dict['adapter_settings']
    langs = adapter_settings.get('sourceLanguages', [])
    for lang in langs:
        try:
            ns = __import__('lang_support', fromlist=[lang])
            getattr(ns, lang).__lldb_init_module(debugger, internal_dict)
        except ImportError:
            pass
        except:
            log.exception('Failed to initialize language support for {}'.format(lang))
