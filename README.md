LLDB Front-End for Visual Studio Code
==========

This extension provides native code debugging in Visual Studio Code via the [LLDB](http://lldb.llvm.org/) debugger engine.

# Pre-requisites
- Visual Studio Code 0.10.7 (January 2016)
- LLDB with Python scripting support.  Make sure it's on you PATH.
  Please refer to [Installing LLDB](#installing-lldb) section for installation instructions.
  Verify installation by running `lldb --version`.

# Using
TL;DR: Press F9 to set breakpoints, press F5 to launch.

See [VS Code Debugging](https://code.visualstudio.com/Docs/editor/debugging) page for general instructions.

## Starting debugging
Create a new [launch configuration](https://code.visualstudio.com/Docs/editor/debugging#_launch-configurations)
with one of the following sets of parameters:

### Launch
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
|`sourceLanguages`|list of strings|| A list of source languages used in the program. This is used only for setting exception breakpoints, since they tend to be language-specific.|

### Attach
|parameter|type|req |         |
|---------|----|:--:|---------|
|`name`   |string|Y| Launch configuration name.|
|`type`   |string|Y| Set to "lldb".|
|`request`|string|Y| Set to "launch".|
|`program`|string|Y| Path to debuggee executable.|
|`pid`    |number|| The process id to attach to.  `pid` may be omitted, in which case the debugger will attempt to locate an already running instance of the program.|
|`stopOnEntry`  |boolean|| Whether to stop debuggee immediately after attaching.|
|`initCommands` |list of strings|| LLDB commands executed upon debugger startup.|
|`preRunCommands`|list of strings|| LLDB commands executed just before attaching.|
|`sourceLanguages`|list of strings|| A list of source languages used in the program. This is used only for setting exception breakpoints, since they tend to be language-specific.|
### stdio configuration
The stdio configuration specifies the connections established for debuggee stdio streams.
Each stream's configuration value may be one of the following:
- `null`: Debugger captures the stream.  Output to `stdout` and `stderr` is sent to debugger console;
  `stdin` is always empty.
- `"/some/path"`: Connects stream to a file, a pipe or a TTY (not supported on Windows).
Hint: to find out the TTY device name for a terminal window, enter `tty` command.
- `"*"`: Creates a new terminal window and connects stream to that terminal.
  On Windows, debuggee is always launched in a new window, however stdio streams may still be configured as described above.


When the `stdio` parameter is assigned a single value, all three streams are configured identically.

It is possible to configure them independently: `"stdio": ["*", null, "/tmp/my.log"]`
will connect stdin to a new terminal, send stdout to debugger console, and stderr - to a log file.

You may also use dictionary syntax: `"stdio": { "stdin": "*", "stdout": null, "stderr": "/tmp/my.log" }`

## Debugger console
VS Code UI does not support all the bells and whistles that the underlying LLDB engine does. To access advanced features
you may enter [LLDB commands](http://lldb.llvm.org/tutorial.html) directly into the debugger console window.
If you would like to evaluate an expression instead, prefix it with '`?`'.

Note that any debugger state changes that you make directly through LLDB commands *will not be reflected in the UI
and will not be persisted across debug sessions*.

# Installing LLDB
## Linux
- On Debian-derived distros (e.g. Ubuntu), run `sudo apt-get install lldb-x.y`, where x.y is LLDB version.
  See [this page](http://llvm.org/apt/) for more info.

## Mac OSX
- [Download](https://developer.apple.com/xcode/download/) and install XCode.
- Install XCode Command Line Tools by running `xcode-select --install`

## Windows
No binary downloads are available at this time.
You are gonna have to [build your own](http://lldb.llvm.org/build.html#BuildingLldbOnWindows).  Sorry :(

# Issues and limitations
- The true locations of resolved breakpoints are not reflected in the UI.
- After a breakpoint stop or stepping, only call stack of the thread where event had occurred will be
  displayed.  To inspect other threads, use the `allthreads` command.  ([VS Code bug #40](https://github.com/Microsoft/vscode/issues/40))
- Global and static variables are not exposed in the UI
