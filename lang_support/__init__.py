import logging
from os import path

log = logging.getLogger(__name__)


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    adapter_settings = internal_dict['adapter_settings']
    langs = set(adapter_settings.get('sourceLanguages', []))
    log.info('languages: {}'.format(langs))
    for lang in langs:
        ns = __import__('lang_support', fromlist=[lang])
        mod = getattr(ns, lang, None)
        if mod is None:
            log.debug('No lang support found for {}'.format(lang))
        else:
            try:
                mod.__lldb_init_module(debugger, internal_dict)
            except Exception as e:
                message = 'Failed to initialize language support for {}'.format(lang)
                log.exception(message)
                print(message, str(e))
