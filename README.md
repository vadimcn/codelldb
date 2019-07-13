CodeLLDB: a LLDB front end for Visual Studio Code
=================================================

# Features
- Supports Linux, macOS and Windows.
- Launch/attach/custom launch.
- Redirection of debuggee's stdio to integrated or external terminal.
- Function, conditional and regex breakpoints, logpoints.
- Flexible launch configurations with settings inheritance.
- Jump to cursor.
- Variable view with customizable formatting.
- Disassembly view.
- Rust language support.
- Python scripting.
- Direct execution of LLDB commands.
- Remote debugging.
- Reverse debugging (experimental, requires compatible backend).

For full details please see the [Users Manual](MANUAL.md).

# Requirements
- 64-bit OS.
- Python 2.7 on Linux and macOS.
- Python 3.6 on Windows.

# Quick Start
Here's a minimal debug configuration to get you started:
```javascript
{
    "name": "Launch",
    "type": "lldb",
    "request": "launch",
    "program": "${workspaceFolder}/<my program>",
    "args": ["-arg1", "-arg2"],
}
```

# Links
- [Users Manual](MANUAL.md)
- [Debugging in VS Code](https://code.visualstudio.com/docs/editor/debugging)
- [Troubleshooting](https://github.com/vadimcn/vscode-lldb/wiki/Troubleshooting)
- [Wiki](https://github.com/vadimcn/vscode-lldb/wiki)
- [Chat](https://gitter.im/vscode-lldb/QnA)


# Screenshots

C++ debugging with data visualization ([Howto](https://github.com/vadimcn/vscode-lldb/wiki/Data-visualization)):<br>
![source](images/plotting.png)
<br>
<br>
Rust debugging:<br>
![source](images/source.png)


