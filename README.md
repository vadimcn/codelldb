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

# Eye Candy

C++ debugging with data visualization ([Howto](https://github.com/vadimcn/vscode-lldb/wiki/Data-visualization)):
<br>
![source](images/plotting.png)
<br>
Rust debugging:
<br>
![source](images/source.png)

# Prerequisites
- Visual Studio Code 1.9.0.
- LLDB with Python scripting support on system PATH. ([Installing LLDB](#installing-lldb))

# Quick Start
Here's a minimal debug configuration to get you started:
```javascript
{
    "name": "Launch",
    "type": "lldb",
    "request": "launch",
    "program": "${workspaceRoot}/<my program>",
    "args": ["-arg1", "-arg2"],
}
```

See also: [Debugging in VS Code](https://code.visualstudio.com/docs/editor/debugging), [the Manual](MANUAL.md).

# [Installing LLDB](https://github.com/vadimcn/vscode-lldb/wiki/Installing-LLDB)
Please see [this page](https://github.com/vadimcn/vscode-lldb/wiki/Installing-LLDB).

# [Manual](MANUAL.md)
Be sure to read the [Fine Manual](MANUAL.md)!

# [Wiki](https://github.com/vadimcn/vscode-lldb/wiki)
Please see the [troubleshooting](https://github.com/vadimcn/vscode-lldb/wiki/Troubleshooting) page
should you have issues with getting the debugger to start.


