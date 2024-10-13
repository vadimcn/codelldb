from typing import Any, Optional
from . import interface
from .event import Event
import lldb

view_id = 0


class Webview:
    '''A simplified interface for [webview panels](https://code.visualstudio.com/api/references/vscode-api#WebviewPanel).'''

    def __init__(self, debugger_id):
        global view_id
        view_id += 1
        self.id = view_id
        self.debugger_id = debugger_id
        self._on_did_receive_message = Event()
        self._on_did_dispose = Event()
        interface.on_did_receive_message.add(self._message_handler)

    def _message_handler(self, message):
        if message.get('id', None) == self.id:
            message_type = message.get('message', None)
            if message_type == 'webviewDidReceiveMessage':
                self._on_did_receive_message.emit(message.get('inner', None))
            elif message_type == 'webviewDidDispose':
                self._on_did_dispose.emit(message.get('inner', None))

    def __del__(self):
        interface.on_did_receive_message.remove(self._message_handler)

    def dispose(self):
        '''Destroy webview panel.'''
        interface.send_message(self.debugger_id, dict(message='webviewDispose', id=self.id))

    def set_html(self, html: str):
        '''Set HTML contents of the webview.'''
        interface.send_message(self.debugger_id, dict(message='webviewSetHtml', id=self.id, html=html))

    def reveal(self,  view_column: Optional[int] = None, preserve_focus: bool = False):
        '''Show the webview panel in a given column.'''
        interface.send_message(self.debugger_id, dict(message='webviewReveal', id=self.id,
                                                      viewColumn=view_column, preserveFocus=preserve_focus))

    def post_message(self, message: Any):
        '''Post a message to the webview content.'''
        interface.send_message(self.debugger_id, dict(message='webviewPostMessage', id=self.id, inner=message))

    @property
    def on_did_receive_message(self) -> Event:
        '''Fired when webview content posts a new message.'''
        return self._on_did_receive_message

    @property
    def on_did_dispose(self) -> Event:
        '''Fired when the webview panel is disposed (either by the user or by calling dispose())'''
        return self._on_did_dispose
