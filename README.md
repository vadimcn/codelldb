# Features
- Debugging on Linux (x64 or ARM), macOS and Windows<sup>*</sup>,
- Conditional breakpoints, function breakpoints, data breakpoints, logpoints,
- Launch debuggee in integrated or external terminal,
- Disassembly view with instruction-level stepping,
- Loaded modules view,
- Python scripting,
- HTML rendering for advanced visualizations,
- Rust language support with built-in visualizars for vectors, strings and other standard types,
- Global and workspace defaults for launch configurations,
- Remote debugging,
- Reverse debugging (experimental, requires compatible backend).

<sup>\*</sup> DWARF debug info format recommended, limited support for MS PDB.

For full details please see [the User's Manual](MANUAL.md).

# Minimal System Requirements
- 64-bit OS
    - Linux: glibc 2.18 (Debian 8, Ubuntu 14.04, Centos 8)
    - Mac: OS X 10.10 Yosemite
    - Windows: 10.0

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
- [Initial Setup](https://github.com/vadimcn/vscode-lldb/wiki/Setup)
- [Debugging in VS Code](https://code.visualstudio.com/docs/editor/debugging) - if you are new to VSCode debugging.
- [CodeLLDB User's Manual](MANUAL.md) - about this specific extension.
- [Troubleshooting](https://github.com/vadimcn/vscode-lldb/wiki/Troubleshooting) - known problems and solutions.
- [Mailing list](https://groups.google.com/g/codelldb-users) - for questions and discussion.


# Screenshots

C++ debugging with data visualization ([Howto](https://github.com/vadimcn/vscode-lldb/wiki/Data-visualization)):<br>
![source](images/plotting.png)
<br>
<br>
Rust debugging:<br>
![source](images/source.png)


