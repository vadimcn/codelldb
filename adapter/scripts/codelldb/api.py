from lldb import SBValue
import warnings
import __main__
from typing import Any, Optional, Union

from . import interface
from .value import Value
from .webview import Webview


def get_config(name: str, default: Any = None) -> Any:
    '''Retrieve a configuration value from the adapter settings.
        name:    Dot-separated path of the setting to retrieve.  For example, `get_config('foo.bar')`,
                 will retrieve the value of `lldb.script.foo.bar` from VSCode configuration.
        default: The default value to return if the configuration value is not found.
    '''
    internal_dict = interface.get_instance_dict(interface.current_debugger())
    settings = internal_dict['adapter_settings'].get('scriptConfig')
    for segment in name.split('.'):
        if settings is None:
            return default
        settings = settings.get(segment)
    return settings


def evaluate(expr: str, unwrap: bool = False) -> Union[Value,  SBValue]:
    '''Performs dynamic evaluation of native expressions returning instances of Value or SBValue.
        expression: The expression to evaluate.
        unwrap: Whether to unwrap the result and return it as lldb.SBValue
    '''
    frame = interface.current_frame()
    value = interface.nat_eval(frame, expr)
    return Value.unwrap(value) if unwrap else value


def wrap(obj: SBValue) -> Value:
    '''Extracts an lldb.SBValue from Value'''
    return obj if type(obj) is Value else Value(obj)


def unwrap(obj: Value) -> SBValue:
    '''Wraps lldb.SBValue in a Value object'''
    return Value.unwrap(obj)


def create_webview(html: Optional[str] = None, title: Optional[str] = None, view_column: Optional[int] = None,
                   preserve_focus: bool = False, enable_find_widget: bool = False,
                   retain_context_when_hidden: bool = False, enable_scripts: bool = False,
                   preserve_orphaned: bool = False):
    '''Create a [webview panel](https://code.visualstudio.com/api/references/vscode-api#WebviewPanel).
        html:               HTML content to display in the webview.  May be later replaced via Webview.set_html().
        title:              Panel title.
        view_column:        Column in which to show the webview.
        preserve_focus:     Whether to preserve focus in the current editor when revealing the webview.
        enable_find_widget: Controls whether the find widget is enabled in the panel.
        retain_context_when_hidden: Controls whether the webview panel retains its context when hidden.
        enable_scripts:     Controls whether scripts are enabled in the webview.
        preserve_orphaned:  Preserve webview panel after the end of the debug session.
    '''
    debugger_id = interface.current_debugger().GetID()
    webview = Webview(debugger_id)
    interface.send_message(debugger_id,
                           dict(message='webviewCreate',
                                id=webview.id,
                                html=html,
                                title=title,
                                viewColumn=view_column,
                                preserveFocus=preserve_focus,
                                enableFindWidget=enable_find_widget,
                                retainContextWhenHidden=retain_context_when_hidden,
                                enableScripts=enable_scripts,
                                preserveOrphaned=preserve_orphaned,
                                )
                           )
    return webview


def debugger_message(output: str, category: str = 'console'):
    debugger_id = interface.current_debugger().GetID()
    interface.fire_event(debugger_id, dict(type='DebuggerMessage', output=output, category=category))


def display_html(html: str, title: Optional[str] = None, position: Optional[int] = None, reveal: bool = False,
                 preserve_orphaned: bool = True):
    '''Display HTML content in a webview panel.
       display_html is **deprecated**, use create_webview instead.
    '''
    inst_dict = interface.get_instance_dict(interface.current_debugger())
    html_webview = inst_dict.get('html_webview')
    if html_webview is None:
        warnings.warn("display_html is deprecated, use create_webview instead", DeprecationWarning)

        html_webview = create_webview(
            html=html,
            title=title,
            view_column=position,
            preserve_focus=not reveal,
            enable_scripts=True,
            preserve_orphaned=preserve_orphaned,
        )

        def on_message(message):
            if message['command'] == 'execute':
                interface.current_debugger().HandleCommand(message['text'])

        def on_disposed(message):
            del globals()['html_webview']

        html_webview.on_did_receive_message.add(on_message)
        html_webview.on_did_dispose.add(on_disposed)
        inst_dict['html_webview'] = html_webview
    else:
        html_webview.set_html(html)
        if reveal:
            html_webview.reveal(view_column=position)


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    debugger.HandleCommand('command script add -c debugger.DebugInfoCommand debug_info')
