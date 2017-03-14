import io
import lldb
import adapter
import base64
import numpy as np
import matplotlib
import matplotlib.pyplot as plt

def show():
    image_bytes = io.BytesIO()
    plt.savefig(image_bytes, format='png', bbox_inches='tight')
    document = '<html><img src="data:data:image/png;base64,%s"></html>' % base64.b64encode(image_bytes.getvalue())
    adapter.preview_html('debugger:/plot', title='Pretty Plot', position=2, content={'debugger:/plot': document})

def plot():
    x = np.linspace(0, 2 * np.pi, 500)
    y1 = np.sin(x)
    y2 = np.sin(3 * x)
    fig, ax = plt.subplots()
    ax.fill(x, y1, 'b', x, y2, 'r', alpha=0.3)
    show()

def plot_image_if(cond, cmap='nipy_spectral_r'):
    if not cond: return False
    xdim = lldb.frame.EvaluateExpression('xdim').GetValueAsSigned()
    ydim = lldb.frame.EvaluateExpression('ydim').GetValueAsSigned()
    image = lldb.frame.EvaluateExpression('image')
    data = image.GetData()
    data = data.ReadRawData(lldb.SBError(), 0, data.GetByteSize())
    data = np.frombuffer(data, dtype=np.int32).reshape((ydim,xdim))
    plt.imshow(data, cmap=cmap, interpolation='nearest')
    show()
