LLDB Front-End for Visual Studio Code
---

# Pre-requisites
- Visual Studio Code 0.10.7 (January 2016)
- LLDB with Python scripting support.  Make sure it's on you PATH.  Verify installation by running `lldb --version`.
Refer to [LLDB Installation](#LLDB-Installation) section instructions.

# Starting debugging
Create a new [launch configuration](https://code.visualstudio.com/Docs/editor/debugging#_launch-configurations)
with one of the following sets of parameters:

## Launching a new process
- Required:
- `name` (string, required): launch configuration name
- `type` (string, required): set to "lldb"
- `request` (string, required): set to "launch"
- `program` (string, required): path to debuggee executable
- `args` (list of strings): command line parameters
- `cwd` (string): working directory
- `env` (dictionary): additional environment variables
- `stdio` (string, list of strings or dictionary): debuggee's stdio configureation (see [below](#stdio-configuration))
- `stopOnEntry` (bool): whether to stop debuggee immediately after launching
- `initCommands` (list of strings): LLDB commands executed upon debugger startup
- `preRunCommands` (list of strings): LLDB commands executed just before launching the program

## Attaching to running process
- `name` (string, required): launch configuration name
- `type` (string, required): set to "lldb"
- `request` (string, required): set to "attach"
- `program` (string, required): path to debuggee executable
- `pid` (number): the process id to attach to.  `pid` may be omitted, in which case the debugger will attempt
  to locate an already running instance of the program.
- `stopOnEntry` (bool): whether to stop debuggee immediately after attaching
- `initCommands` (list of strings): LLDB commands executed upon debugger startup
- `preRunCommands` (list of strings): LLDB commands executed just before attaching

## stdio configuration
The stdio configuration specifies where the debuggee's stdio streams will be connected to:
- `null`: debugger captures the stream.  Output to `stdout` and `stderr` is sent to the debugger console;
  `stdin` is always empty.
- `"string"`: connects stream to a file, a pipe or a TTY (not supported on Windows).
Hint: to find out a TTY device name, run the `tty` command.
- `"*"`: start a new terminal window and connect stream to its slave TTY device.

On Windows debuggee is always launched in a new window, however stdio streams may still be configured as described above.

- When the value of the `stdio` attribute is a string, all three streams are configured identically.
- It is also possible to configure them independently: `"stdio": ["*", null, "/tmp/my.log"]`
will connect stdin to a new terminal, send stdout to debugger console, and stderr - to a log file.
- You can also use dictionary syntax: `"stdio": { "stdin": "*", "stdout": null, "stderr": "/tmp/my.log" }`

# Debugger console
You may enter [LLDB commands](http://lldb.llvm.org/tutorial.html) directly into the VS Code debugger console window.
If you would like to evaluate an expression instead, prefix it with '`?`'.

Note that any debugger state changes that you make directly through LLDB commands *will not be reflected in the UI
and will not be persisted across debug sessions*.

# LLDB Installation
## Linux
Run `sudo apt-get install lldb-x.y`, where x.y is the current lldb version.

See [this page](http://llvm.org/apt/) for more info.

## Mac OSX
- Install XCode from [Apple Downloads](https://developer.apple.com/xcode/download/)
- Install XCode Command Line Tools by running `xcode-select --install`

## Windows
Build your own :(

# Bugs and issues
- After a breakpoint stop or stepping, only callstack of the thread where this event had occurred will be
  displayed ([VS Code bug #40](https://github.com/Microsoft/vscode/issues/40)).  To inspect other threads,
  use the `allthreads` command.
