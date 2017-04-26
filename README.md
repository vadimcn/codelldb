CodeLLDB: a LLDB front end for Visual Studio Code
=================================================

# Features
- Supports Linux, macOS and Windows (with caveats - see below).
- Launch processes with configurable stdio redirection.
- Attach to processes by pid or by name.
- Scripted custom launch for ultimate flexibility.
- Function, conditional and regex breakpoints.
- Disassembly View.
- LLDB commands and expression evaluation in Debug Console.
- Configurable result formatting.
- Display of HTML content.
- Rust language support.

Please see the [Manual](MANUAL.md) for details.

![source](images/plotting.png)
![source](images/source.png)

# Prerequisites
- Visual Studio Code 1.9.0.
- LLDB with Python scripting support on system PATH. ([Installing LLDB](#installing-lldb))

# Debugging
Here's a minimal configuration to get you started:
```javascript
{
    "name": "Launch",
    "type": "lldb",
    "request": "launch",
    "program": "${workspaceRoot}/<my program>",
    "args": ["-arg1", "-arg2"],
}
```

See also: [Debugging in VS Code](https://code.visualstudio.com/docs/editor/debugging).

# Installing LLDB
## Linux
On Debian-derived distros (e.g. Ubuntu), run `sudo apt-get install python-lldb-x.y`, where x.y is the LLDB version.<br>
See [this page](http://lldb.llvm.org/download.html) for installing nightlies.

Note: Some distros install LLDB with a versioned name, e.g. `lldb-4.0`.  In this case you will need to
configure LLDB executable name via [Workspace Configuration](MANUAL.md#workspace-configuration).

## MacOS
- [Download](https://developer.apple.com/xcode/download/) and install XCode.
- Install XCode Command Line Tools by running `xcode-select --install`

### **Note**
LLDB is incompatible with Brew or MacPorts-installed Python.  If you have one installed on your machine,
please read [this](https://github.com/vadimcn/vscode-lldb/wiki/Troubleshooting#is-lldbs-python-scripting-functional).

## Windows
- [Download](http://llvm.org/builds/) and install LLVM for Windows.
- [Download](https://www.python.org/downloads/windows/) and install Python 3.5.x. If you've
installed a 64-bit LLVM (not recommended), you will need a 64-bit Python as well.
- Make sure that both LLDB and Python install directories are on the PATH.

### **Note**
At the moment, LLDB's support of Microsoft PDB debug info format is rather poor.  Also, the
64-bit Windows LLDB is known to be buggy.<br>
This means that in practice it's only useful for debugging 32-bit binaries produced by GNU
toolchains.  This situation will hopefully improve with time.

# [Manual](MANUAL.md)
Be sure to read the [Fine Manual](MANUAL.md)!

# [Wiki](https://github.com/vadimcn/vscode-lldb/wiki)
Please see the [troubleshooting](https://github.com/vadimcn/vscode-lldb/wiki/Troubleshooting) page
should you have issues with getting the debugger to start.


