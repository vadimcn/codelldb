import matplotlib.pyplot as plt
import io
import lldb
import debugger
import base64
import numpy as np
import matplotlib
matplotlib.use('agg')


def show():
    image_bytes = io.BytesIO()
    plt.savefig(image_bytes, format='png', bbox_inches='tight')
    document = '<html><img src="data:image/png;base64,%s"></html>' % base64.b64encode(
        image_bytes.getvalue()).decode('utf-8')
    debugger.display_html(document, position=2)


def plot_image(image, xdim, ydim, cmap='nipy_spectral_r'):
    image = debugger.unwrap(image)
    if image.TypeIsPointerType():
        image_addr = image.GetValueAsUnsigned()
    else:
        image_addr = image.AddressOf().GetValueAsUnsigned()
    data = lldb.process.ReadMemory(image_addr, int(xdim * ydim) * 4, lldb.SBError())
    data = np.frombuffer(data, dtype=np.int32).reshape((ydim, xdim))
    plt.imshow(data, cmap=cmap, interpolation='nearest')
    show()
    return True


def display(x):
    print(repr(x))


def display_html_test():
    document = '''<html><script>
                let vscode = acquireVsCodeApi();
                vscode.postMessage({'command': 'execute', 'text': 'script debugvis.display_html_callback("foo")'});
                </script></html>'''
    debugger.display_html(document)


def display_html_callback(args):
    print('display_html_callback', args)


def webview_test():
    document = '''<html><script>
                let vscode = acquireVsCodeApi();
                window.addEventListener('message', event => vscode.postMessage(event.data));
                vscode.postMessage({foo: 'foo'});
                </script></html>'''
    wv = debugger.create_webview(document, title='hren', enable_scripts=True)
    wv.on_did_receive_message.add(webview_callback)
    wv.post_message(dict(bar='bar'))


def webview_callback(args):
    print('webview_callback', args)
