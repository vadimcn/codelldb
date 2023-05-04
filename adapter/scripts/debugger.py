import lldb
from codelldb import codelldb
from codelldb.value import Value
from codelldb.webview import Webview
from codelldb.debug_info import DebugInfoCommand  # pyright: ignore

__all__ = ['evaluate', 'wrap', 'unwrap', 'display_html', 'create_webview']


def evaluate(expr, unwrap=False):
    value = codelldb.nat_eval(lldb.frame, expr)
    return Value.unwrap(value) if unwrap else value


def wrap(obj):
    return obj if type(obj) is Value else Value(obj)


def unwrap(obj):
    return Value.unwrap(obj)


def create_webview(html=None, title=None, view_column=None, preserve_focus=False,
                   enable_find_widget=False, retain_context_when_hidden=False,
                   enable_scripts=False):
    webview = Webview()
    codelldb.send_message(dict(message='webviewCreate',
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


def display_html(html, title=None, position=None, reveal=False):
    global html_webview
    if html_webview is None:
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

        def on_dispoed(message):
            global html_webview
            html_webview = None

        html_webview.on_did_receive_message.add(on_message)
        html_webview.on_did_dispose.add(on_dispoed)
    else:
        html_webview.set_html(html)
        if reveal:
            html_webview.reveal(view_column=position)


html_webview = None


def __lldb_init_module(debugger, internal_dict):  # pyright: ignore
    debugger.HandleCommand('command script add -c debugger.DebugInfoCommand debug_info')
