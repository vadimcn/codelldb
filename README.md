# Features
- Debugging on Linux (x86 or Arm), MacOS (x86 or Arm) and Windows<sup>\*</sup> (x86 only),
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

For full details please see the [User's Manual](MANUAL.md).<br>

<sup>\*</sup> For a good debugging experience on Windows, please use `x86_64-pc-windows-gnu` compilation target.
MS PDB debug info support is limited, especially for Rust binaries. [More info.](https://github.com/vadimcn/vscode-lldb/wiki/Windows)

# Supported Platforms
- Linux with glibc 2.18+ (e.g. Debian 8, Ubuntu 14.04, Centos 8) for x86_64, aarch64 or armhf architecture,
- MacOS X 10.10+ for x86_64 and 11.0+ for arm64 architecture,
- Windows 10 for x86_64 architecture.

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
- [Debugging in VS Code](https://code.visualstudio.com/docs/editor/debugging) - if you are new to VSCode debugging.
- [CodeLLDB User's Manual](MANUAL.md) - how to use this extension.
- [LLDB Homepage](https://lldb.llvm.org/) - all of LLDB's CLI commands and scripting features can be used too.
- [Wiki pages](https://github.com/vadimcn/vscode-lldb/wiki) - [troubleshooting](https://github.com/vadimcn/vscode-lldb/wiki/Troubleshooting) and other tips and tricks.
- [Discussions](https://github.com/vadimcn/vscode-lldb/discussions) - for questions and discussion.

# Screenshots

C++ debugging with data visualization ([Howto](https://github.com/vadimcn/vscode-lldb/wiki/Data-visualization)):<br>
![source](images/plotting.png)
<br>
<br>
Rust debugging:<br>
![source](images/source.png)


