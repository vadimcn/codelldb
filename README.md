# Features
- Conditional breakpoints, function breakpoints, logpoints,
- Hardware data access breakpoints (watchpoints),
- Launch debuggee in integrated or external terminal,
- Disassembly view with instruction-level stepping,
- [Step Into Targets](https://code.visualstudio.com/updates/v1_46#_step-into-targets).
- Caller exclusion for breakpoints.
- Memory view.
- Loaded modules view,
- Python scripting,
- HTML rendering for advanced visualizations,
- Workspace-level defaults for launch configurations,
- Remote debugging,
- Reverse debugging (experimental, requires a compatible backend).

For full details please see [User's Manual](MANUAL.md).<br>

# Languages
The primary focus of this project are the C++ and Rust languages, for which CodeLLDB includes built-in visualizers for
vectors, strings, maps, and other standard library types.<br>
That said, it is usable with most other compiled languages whose compiler generates compatible debugging information,
such as Ada, Fortran, Kotlin Native, Nim, Objective-C, Pascal, [Swift](https://github.com/vadimcn/codelldb/wiki/Swift)
and Zig.

# Supported Platforms

## Host
- Linux with glibc 2.18+ for x86_64, aarch64 or armhf.
- MacOS 10.12+ for x86_64 and 11.0+ for arm64.
- Windows 10 and 11 for x86_64. [See Windows notes in wiki!](https://github.com/vadimcn/codelldb/wiki/Windows)

## Target
CodeLLDB supports AArch64, ARM, AVR, MSP430, RISCV, X86 architectures and may be used to debug on embedded platforms
via [remote debugging](MANUAL.md#remote-debugging).

# More information
- [CodeLLDB User's Manual](MANUAL.md) - how to use this extension.
- [Debugging in VS Code](https://code.visualstudio.com/docs/editor/debugging) - if you are new to VSCode debugging.
- [LLDB Tutorial](https://lldb.llvm.org/use/tutorial.html) - all of LLDB's CLI commands and scripting features may be used in CodeLLDB.
- [Wiki pages](https://github.com/vadimcn/codelldb/wiki) - [troubleshooting](https://github.com/vadimcn/codelldb/wiki/Troubleshooting) and other tips and tricks.
- [Discussions](https://github.com/vadimcn/codelldb/discussions) - for questions and discussions.

# Screenshots

C++ debugging with data visualization ([Howto](https://github.com/vadimcn/codelldb/wiki/Data-visualization)):<br>
![source](images/plotting.png)
<br>
<br>
Rust debugging:<br>
![source](images/source.png)


