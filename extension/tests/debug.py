import io
import lldb
import debugger
import base64
import numpy as np
import matplotlib
import matplotlib.pyplot as plt

def show():
    image_bytes = io.BytesIO()
    plt.savefig(image_bytes, format='png', bbox_inches='tight')
    document = '<html><img src="data:data:image/png;base64,%s"></html>' % base64.b64encode(image_bytes.getvalue())
    debugger.display_html('debugger:/plot', title='Pretty Plot', position=2, content={'debugger:/plot': document})

def show2():
    image_bytes = io.BytesIO()
    plt.savefig(image_bytes, format='png', bbox_inches='tight')
    document = '<html><img src="data:data:image/png;base64,%s"></html>' % base64.b64encode(image_bytes.getvalue())
    debugger.register_content_provider(lambda uri: document)
    debugger.display_html('debugger:/plot', title='Pretty Plot', position=2)

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

def test():
    x = np.linspace(0, 1, 500)
    y = np.sin(4 * np.pi * x) * np.exp(-5 * x)
    fig, ax = plt.subplots()
    ax.fill(x, y, zorder=10)
    ax.grid(True, zorder=5)
    show2()
