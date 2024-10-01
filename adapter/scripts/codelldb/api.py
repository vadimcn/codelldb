import lldb
import warnings
import __main__
from typing import Any, Optional, Union

from . import interface
from .value import Value
from .webview import Webview


def get_config(name: str, default: Any = None) -> Any:
    '''Retrieve a configuration value from the adapter settings.
        name:    Dot-separated path of the setting to retrieve.  For example, 'foo.bar', will retrieve the value of `lldb.script.foo.bar`.
        default: The default value to return if the configuration value is not found.
    '''
    internal_dict = interface.get_instance_dict(lldb.debugger)
    settings = internal_dict['adapter_settings'].get('scriptConfig')
    for segment in name.split('.'):
        if settings is None:
            return default
        settings = settings.get(segment)
    return settings


def evaluate(expr: str, unwrap: bool = False) -> Union[Value,  lldb.SBValue]:
    '''Performs dynamic evaluation of native expressions returning instances of Value or SBValue.
        expression: The expression to evaluate.
        unwrap: Whether to unwrap the result and return it as lldb.SBValue
    '''
    value = interface.nat_eval(lldb.frame, expr)
    return Value.unwrap(value) if unwrap else value


def wrap(obj: lldb.SBValue) -> Value:
    '''Extracts an lldb.SBValue from Value'''
    return obj if type(obj) is Value else Value(obj)


def unwrap(obj: Value) -> lldb.SBValue:
    '''Wraps lldb.SBValue in a Value object'''
    return Value.unwrap(obj)


def create_webview(html: Optional[str] = None, title: Optional[str] = None, view_column: Optional[int] = None,
                   preserve_focus: bool = False, enable_find_widget: bool = False,
                   retain_context_when_hidden: bool = False, enable_scripts: bool = False):
    '''Create a [webview panel](https://code.visualstudio.com/api/references/vscode-api#WebviewPanel).
        html:               HTML content to display in the webview.  May be later replaced via Webview.set_html().
        title:              Panel title.
        view_column:        Column in which to show the webview.
        preserve_focus:     Whether to preserve focus in the current editor when revealing the webview.
        enable_find_widget: Controls if the find widget is enabled in the panel.
        retain_context_when_hidden: Controls if the webview panel retains its context when it is hidden.
        enable_scripts:     Controls if scripts are enabled in the webview.
    '''
    webview = Webview()
    interface.send_message(dict(message='webviewCreate',
                                id=webview.id,
                                html=html,
                                title=title,
                                viewColumn=view_column,
                                preserveFocus=preserve_focus,
                                enableFindWidget=enable_find_widget,
                                retainContextWhenHidden=retain_context_when_hidden,
                                enableScripts=enable_scripts
                                ))
    return webview


def display_html(html: str, title: Optional[str] = None, position: Optional[int] = None, reveal: bool = False):
    '''Display HTML content in a webview panel.
       display_html is **deprecated**, use create_webview instead.
    '''
    global html_webview
    if html_webview is None:
        warnings.warn("display_html is deprecated, use create_webview instead", DeprecationWarning)

        html_webview = create_webview(
            html=html,
            title=title,
            view_column=position,
            preserve_focus=not reveal,
            enable_scripts=True
        )

        def on_message(message):
            if message['command'] == 'execute':
                lldb.debugger.HandleCommand(message['text'])

        def on_disposed(message):
            global html_webview
            html_webview = None

        html_webview.on_did_receive_message.add(on_message)
        html_webview.on_did_dispose.add(on_disposed)
    else:
        html_webview.set_html(html)
        if reveal:
            html_webview.reveal(view_column=position)


html_webview = None


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    debugger.HandleCommand('command script add -c debugger.DebugInfoCommand debug_info')
