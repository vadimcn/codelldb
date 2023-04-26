import io
import lldb
import debugger
import base64
import numpy as np
import matplotlib
matplotlib.use('agg')
import matplotlib.pyplot as plt

def show():
    image_bytes = io.BytesIO()
    plt.savefig(image_bytes, format='png', bbox_inches='tight')
    document = '<html><img src="data:image/png;base64,%s"></html>' % base64.b64encode(image_bytes.getvalue()).decode('utf-8')
    debugger.display_html(document, position=2)

def plot_image(image, xdim, ydim, cmap='nipy_spectral_r'):
    image = debugger.unwrap(image)
    if image.TypeIsPointerType():
        image_addr = image.GetValueAsUnsigned()
    else:
        image_addr = image.AddressOf().GetValueAsUnsigned()
    data = lldb.process.ReadMemory(image_addr, int(xdim * ydim) * 4, lldb.SBError())
    data = np.frombuffer(data, dtype=np.int32).reshape((ydim,xdim))
    plt.imshow(data, cmap=cmap, interpolation='nearest')
    show()

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
