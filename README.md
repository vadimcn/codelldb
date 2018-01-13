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
- Rust language support (excluding LLDB 3.8).

For full details please see the [Manual](MANUAL.md).

# Eye Candy

C++ debugging with data visualization ([Howto](https://github.com/vadimcn/vscode-lldb/wiki/Data-visualization)):
![source](images/plotting.png)
<br>
Rust debugging:
![source](images/source.png)

# Prerequisites
- Visual Studio Code 1.15.0.
- LLDB with Python scripting support on system PATH. ([Installing LLDB](https://github.com/vadimcn/vscode-lldb/wiki/Installing-LLDB))

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

# Links
- [Installing LLDB](https://github.com/vadimcn/vscode-lldb/wiki/Installing-LLDB)
- [The Fine Manual](MANUAL.md)
- [Wiki](https://github.com/vadimcn/vscode-lldb/wiki)
- [Chat](https://gitter.im/vscode-lldb/QnA)
