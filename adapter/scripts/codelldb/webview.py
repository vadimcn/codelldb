from . import codelldb
from .event import Event

view_id = 0


class Webview:
    def __init__(self):
        global view_id
        view_id += 1
        self.id = view_id
        self.on_did_receive_message = Event()
        self.on_did_dispose = Event()
        codelldb.on_did_receive_message.add(self._message_handler)

    def _message_handler(self, message):
        if message.get('id', None) == self.id:
            message_type = message.get('message', None)
            if message_type == 'webviewDidReceiveMessage':
                self.on_did_receive_message.emit(message.get('inner', None))
            elif message_type == 'webviewDidDispose':
                self.on_did_dispose.emit(message.get('inner', None))

    def __del__(self):
        codelldb.on_did_receive_message.remove(self._message_handler)

    def dispose(self):
        codelldb.send_message(dict(message='webviewDispose', id=self.id))

    def set_html(self, html):
        codelldb.send_message(dict(message='webviewSetHtml', id=self.id, html=html))

    def reveal(self,  view_column=None, preserve_focus=False):
        codelldb.send_message(dict(message='webviewReveal', id=self.id,
                                   viewColumn=view_column, preserveFocus=preserve_focus))

    def post_message(self, message):
        codelldb.send_message(dict(message='webviewPostMessage', id=self.id, inner=message))
