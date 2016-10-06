CodeLLDB: a LLDB front end for Visual Studio Code
=================================================

[View this readme on GitHub](https://github.com/vadimcn/vscode-lldb/blob/0.3.0/README.md) (working hyperlinks!)

[See what's new.](#whats-new)

# Features
- [Attach](#attaching) or [Launch](#launching)
- Redirect [debuggee stdio](#stdio) to a file or a terminal.
- Breakpoints ([function](https://code.visualstudio.com/Docs/editor/debugging#_function-breakpoints), conditional, [regex](#regex-breakpoints))
- [Disassembly View](#disassembly-view)
- Line or instruction stepping
- Hover tips
- Watch
- Multiple threads
- [Configurable variable formatting](#formatting)
- [LLDB commands](#lldb-commands)
- [Expression evaluation](#expressions)
- [Rust language support](#rust-language-support)

# Prerequisites
- Visual Studio Code 1.5.0.
- LLDB with Python scripting support on system PATH. ([Installing LLDB](#installing-lldb))

# Debugging

See [VS Code Debugging](https://code.visualstudio.com/Docs/editor/debugging) page for general instructions.

## Configuration
Create a new [launch configuration](https://code.visualstudio.com/Docs/editor/debugging#_launch-configurations)
to either launch your program or attach to already running process:

### Launching
|parameter |type|req |         |
|----------|----|:--:|---------|
|`name`    |string|Y| Launch configuration name.|
|`type`    |string|Y| Set to "lldb".|
|`request` |string|Y| Set to "launch".|
|`program` |string|Y| Path to debuggee executable.|
|`args`    |string &#124; list of strings|| Command line parameters.  If this is a string, it will be split using shell-like syntax.|
|`cwd`     |string|| Working directory.|
|`env`     |dictionary|| Additional environment variables.  Tip: you may refer to existing environment variables like so: `${env.VARIABLE}`.|
|`stdio`   |string &#124; list &#124; dictionary|| Stdio configuration (see [below](#stdio)).|
|`terminal`|string|| Destination for debuggee's stdio streams: `console` (default) for Debug Console, `integrated` for VSCode integrated terminal, `external` for a new terminal window.|
|`stopOnEntry`  |boolean|| Whether to stop debuggee immediately after launching.|
|`initCommands` |list of strings|| LLDB commands executed upon debugger startup.|
|`preRunCommands`|list of strings|| LLDB commands executed just before launching the program.|
|`sourceLanguages`|list of strings|| A list of source languages used in the program. This is used for setting exception breakpoints, since they tend to be language-specific.|

### Attaching

Note that attaching to a running process may be [restricted](https://en.wikipedia.org/wiki/Ptrace#Support)
on some systems.  You may need to adjust system configuration to enable it.

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
The `stdio` property is a list of redirection targets for each of debuggee's stdio streams: 
- `null` (default) will connect the stream to a terminal (as specified by the `terminal` launch property)<sup>1</sup>.
- `"/some/path"` will cause the stream to be redirected to the specified file, pipe or TTY device <sup>2</sup>.

For example, `"stdio": [null, null, "/tmp/my.log"]` will connect stdin and stdout to a terminal, while sending
stderr to the a file.
- You may also use dictionary syntax (`"stdio": { "stdin": null, "stdout": null, "stderr": "/tmp/my.log" }`).
- A scalar value will configure all three streams identically (`"stdio": null`).

<sup>1</sup> On Windows debuggee is always launched in a new window, however stdio streams may still be redirected
as described above.  
<sup>2</sup> Use `tty` command inside a terminal window to find out its TTY device path.  

## Regex Breakpoints
When setting a function breakpoint, if the first character of the function name is '`/`',
the rest of the string is interpreted as a regular expression.  This shall cause a breakpoint to
be set in every function matching the expression (the list of locations may be examined
using `break list` command).

## Disassembly View
When stepping into a compile unit which does not have a debug info, CodeLLDB will instead display
disassembly of the current function.  This behavior may be controlled using `LLDB: Show Disassembly`
and `LLDB: Toggle Disassembly` commands.  The former allows to choose between `never`,
`auto` (the default) and `always`, the latter toggles between `auto` and `always`.

When is disassembly view, the 'step over' and 'step into' debug actions will step by instruction
instead of by line.

## Formatting
You may change the default display format of variables using the `LLDB: Display Format` command.
When evaluating expressions from Debug Console or in the 'Watch' view, you may also control
formatting of individual expressions by adding a suffix. For example `$rax,x` will format the value
as hex. Here's the full list:

|suffix  |format |
|--------|-------|
|`x`     | Hex |
|`o`     | Octal |
|`d`     | Decimal |
|`u`     | Unsigned decimal |
|`b`     | Binary |
|`f`     | Float (reinterprets bits, no casting is done) |
|`p`     | Pointer |
|`s`     | C string |
|`y`     | Bytes |
|`Y`     | Bytes with ASCII |


## LLDB Commands
VS Code UI does not provide access to all the bells and whistles of the underlying LLDB engine. To access advanced features
you may enter [LLDB commands](http://lldb.llvm.org/tutorial.html) into Debug Console.
If you would like to evaluate an expression instead, prefix it with '`?`'.

Note that any debugger state changes that you make directly through LLDB commands will not be reflected in the UI
and will not be persisted across debug sessions.

## Expressions
*(New in v0.3.0)* CodeLLDB leverages Python interpreter to evaluate expressions in Debug Console and the Watch view.
The debuggee variables are represented by a special wrapper class that implements 
most of the usual Python operators on top of the view provided by LLDB variable formatters.
This means that things like indexing a `std::vector` with an integer, or comparing a `std::string` 
to a string literal, just work!  
Unlike regular Python scripts, though, all identifiers are interpreted as variable names. If you need
to use an actual Python keyword, prefix it with '@'.  For example: `[sqrt(x) @for x @in y]`.

## C++ Expressions
You may *also* use the LLDB's built-in C++ expression evaluator.  Just add an extra `?` in front of 
the expression (i.e. `??` in Debug Console and `?` in Watch).  Note, however, that C++ evaluator
ignores variable formatters, so you will have to operate on raw data structures.

# Rust Language Support

CodeLLDB supports visualization of most common Rust data types:
- Built-in types: tuples, enums, arrays, array and string slices.
- Standard library types: `Vec`, `String`, `CString`, `OSString`.

To enable this feature, add `"sourceLanguages": ["rust"]` into your launch configuration.

Note: There is a known incompatibility of debug info emitted by `rustc` and LLDB 3.8:
you won't be able to step through code or inspect variables if you have this version.
The workaround is to use either LLDB 3.7 or 3.9.  On OSX, LLDB shipped with Xcode 8 is known to
have this problem fixed.

# Installing LLDB
## Linux
On Debian-derived distros (e.g. Ubuntu), run `sudo apt-get install python-lldb-x.y`, where x.y is the LLDB version.
You may need to create symlinks to `lldb` and `lldb-server` manually.

See [this page](http://lldb.llvm.org/download.html) for installing nightlies.

## Mac OSX
- [Download](https://developer.apple.com/xcode/download/) and install XCode.
- Install XCode Command Line Tools by running `xcode-select --install`

## Windows
No binary downloads are available at this time.
You are gonna have to [build your own](http://lldb.llvm.org/build.html#BuildingLldbOnWindows).  Sorry :(

# [Troubleshooting](https://github.com/vadimcn/vscode-lldb/wiki/Troubleshooting)

# What's New?

## 0.3.0
- [Variable visualizers for Rust](#rust-language-support).
- New [expression evaluator](#expressions).
- Bug fixes.

## 0.2.2
- Bug fixes.

## 0.2.1
- Added 'terminal' launch config option. '*' in stdio config now behaves identically to null.
- Moved static variables out to their own scope.
- Disassembly in symbolless locations should work now.
- Resume debuggee after attach, unless stopOnEntry is true.

## 0.2.0
- Added [disassembly view](#disassembly-view).
- Added [variable formatting](#formatting).

## 0.1.3
- Added support for setting variable values (primitive types only).
- Added [regex breakpoints](#regex-breakpoints).

## 0.1.2
- Infer `.exe` target extension on Windows.
- `args` may now be a string.

## 0.1.0
First released version.
