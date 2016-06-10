LLDB Front-End for Visual Studio Code
=====================================

Native debugging in Visual Studio Code via [LLDB debugger engine](http://lldb.llvm.org/).

Features:
- Attach or Launch
- Breakpoints (function, conditional)
- Expression evaluation
- Hover Tips
- Watch
- Call Stacks
- Multiple Threads
- Stepping
- LLDB Commands

# Prerequisites
- Visual Studio Code 1.1.1.
- LLDB with Python scripting support. ([Installing LLDB](#installing-lldb))

# Debugging

See [VS Code Debugging](https://code.visualstudio.com/Docs/editor/debugging) page for general instructions.

## Configuration
Create a new [launch configuration](https://code.visualstudio.com/Docs/editor/debugging#_launch-configurations)
to either launch your program or attach to already running process:

### Launching
|parameter|type|req |         |
|---------|----|:--:|---------|
|`name`   |string|Y| Launch configuration name.|
|`type`   |string|Y| Set to "lldb".|
|`request`|string|Y| Set to "launch".|
|`program`|string|Y| Path to debuggee executable.|
|`args`   |list of strings|| Command line parameters.|
|`cwd`    |string|| Working directory.|
|`env`    |dictionary|| Additional environment variables.|
|`stdio`  |string, list or dictionary|| Debuggee's stdio configureation (see [below](#stdio-configuration)).|
|`stopOnEntry`  |boolean|| Whether to stop debuggee immediately after launching.|
|`initCommands` |list of strings|| LLDB commands executed upon debugger startup.|
|`preRunCommands`|list of strings|| LLDB commands executed just before launching the program.|
|`sourceLanguages`|list of strings|| A list of source languages used in the program. This is used for setting exception breakpoints, since they tend to be language-specific.|

### Attaching

Note that attaching to a running process may be [restricted](https://en.wikipedia.org/wiki/Ptrace#Support)
on some systems.  You may need to adjust system configuration to enable attaching.

|parameter|type|req |         |
|---------|----|:--:|---------|
|`name`   |string|Y| Launch configuration name.|
|`type`   |string|Y| Set to "lldb".|
|`request`|string|Y| Set to "attach".|
|`program`|string|Y| Path to debuggee executable.|
|`pid`    |number|| The process id to attach to.  `pid` may be omitted, in which case the debugger will attempt to locate an already running instance of the program.|
|`stopOnEntry`  |boolean|| Whether to stop debuggee immediately after attaching.|
|`initCommands` |list of strings|| LLDB commands executed upon debugger startup.|
|`preRunCommands`|list of strings|| LLDB commands executed just before attaching.|
|`sourceLanguages`|list of strings|| A list of source languages used in the program. This is used for setting exception breakpoints, since they tend to be language-specific.|

### Stdio
The stdio configuration specifies connections established for debuggee's stdio streams.
Each stream's configuration value may be one of the following:

|value         |         |
|--------------|---------
|`null`        | Debugger captures the stream, `stdout` and `stderr` output are sent to debugger console; `stdin` is always empty.|
|`"*"`         | Creates a new terminal window and connects stream to that terminal.|
|`"/some/path"`| Connects stream to a file, a pipe or a TTY (not supported on Windows). Hint: use `tty` command to find out the TTY device name for a terminal window.|

Fro example, `"stdio": ["*", null, "/tmp/my.log"]` will connect `stdin` to a new terminal, send `stdout` to debugger console,
and `stderr` - to a log file.
- You may also use dictionary syntax: `"stdio": { "stdin": "*", "stdout": null, "stderr": "/tmp/my.log" }`.
- A single value will configure all three streams identically: `"stdio": "*"`.

On Windows debuggee is always launched in a new window, however stdio streams may still be configured as described above.

## LLDB Commands
VS Code UI does not support all the bells and whistles that the underlying LLDB engine does. To access advanced features
you may enter [LLDB commands](http://lldb.llvm.org/tutorial.html) directly into the debugger console window.
If you would like to evaluate an expression instead, prefix it with '`?`'.

Note that any debugger state changes that you make directly through LLDB commands *will not be reflected in the UI
and will not be persisted across debug sessions*.

# Installing LLDB
## Linux
- On Debian-derived distros (e.g. Ubuntu), run `sudo apt-get install lldb-x.y`, where x.y is the LLDB version.
  See [this page](http://lldb.llvm.org/download.html) for more info.

## Mac OSX
- [Download](https://developer.apple.com/xcode/download/) and install XCode.
- Install XCode Command Line Tools by running `xcode-select --install`

## Windows
No binary downloads are available at this time.
You are gonna have to [build your own](http://lldb.llvm.org/build.html#BuildingLldbOnWindows).  Sorry :(


# Release Notes

## 0.1.0
First released version.
