
# Table of Contents

- [Starting a Debug Session](#starting-a-debug-session)
    - [Launching](#launching)
        - [Configuring Stdio](#stdio)
    - [Attaching](#attaching)
    - [Custom Launch](#custom-launch)
    - [Remote Debugging](#remote-debugging)
    - [Loading a Core Dump](#loading-a-core-dump)
    - [Source Path Remapping](#source-path-remapping)
    - [Parameterized Launch Configurations](#parameterized-launch-configurations)
- [Debugger Features](#debugger-features)
    - [Commands](#commands)
    - [Regex Breakpoints](#regex-breakpoints)
    - [Conditional Breakpoints](#conditional-breakpoints)
    - [Disassembly View](#disassembly-view)
    - [Formatting](#formatting)
        - [Pointers](#pointers)
    - [Expressions](#expressions)
    - [Debugger API](#debugger-api)
- [Alternate LLDB backends](#alternate-lldb-backends)
- [Rust Language Support](#rust-language-support)
- [Workspace Configuration](#workspace-configuration)

# Starting a Debug Session

To start a debug session you will need to create a [launch configuration](https://code.visualstudio.com/Docs/editor/debugging#_launch-configurations) for your program:

## Launching

Flow during the launch sequence:
1. Debug session is created.
2. The `initCommands` sequence is executed.
3. Debug target is created using launch configurtion prameters (`program`, `args`, `env`, `cwd`, `stdio`).
4. Breakpoints are set.
5. The `preRunCommands` sequence is executed.  These commands may alter debug target configuration.
6. Debuggee is launched.
7. The `postRunCommands` sequence is executed.

At the end of the debug session `exitCommands` sequence is executed.

|parameter          |type|req |         |
|-------------------|----|:--:|---------|
|**name**           |string|Y| Launch configuration name.
|**type**           |string|Y| Set to `lldb`.
|**request**        |string|Y| Set to `launch`.
|**program**        |string|Y| Path to the debuggee executable.
|**cargo**          |string|Y| See [Cargo support](#cargo-support).
|**args**           |string &#10072; [string]| | Command line parameters.  If this is a string, it will be split using shell-like syntax.
|**cwd**            |string| | Working directory.
|**env**            |dictionary| | Additional environment variables.  You may refer to existing environment variables using `${env:NAME}` syntax, for example `"PATH" : "${env:HOME}/bin:${env:PATH}"`.
|**stdio**          |string &#10072; [string] &#10072; dictionary| | See [Stdio Configuration](#stdio).
|**terminal**       |string| | Destination for debuggee's stdio streams: <ul><li>`console` (default) for Debug Console</li><li>`integrated` for VSCode integrated terminal</li><li>`external` for a new terminal window</li></ul>
|**stopOnEntry**    |boolean| | Whether to stop debuggee immediately after launching.
|**initCommands**   |[string]| | LLDB commands executed upon debugger startup.
|**preRunCommands** |[string]| | LLDB commands executed just before launching the debuggee.
|**postRunCommands**|[string]| | LLDB commands executed just after launching the debuggee.
|**exitCommands**   |[string]| | LLDB commands executed at the end of debugging session.
|**expressions**    |string| | The default expression evaluator type: `simple`, `python` or `native`.  See [Expressions](#expressions).
|**sourceMap**      |dictionary| | See [Source Path Remapping](#source-path-remapping).
|**sourceLanguages**| A list of source languages used in the program.  This is used to enable language-specific debugger features.

### Stdio
The **stdio** property is a list of redirection targets for each of debuggee's stdio streams:
- `null` (default) will connect stream to a terminal (as specified by the **terminal** launch property)<sup>1</sup>.
- `"/some/path"` will cause stream to be redirected to the specified file, pipe or a TTY device <sup>2</sup>.

For example, `"stdio": [null, null, "/tmp/my.log"]` will connect stdin and stdout to a terminal, while sending
stderr to the specified file.
- A scalar value will configure all three streams identically: `"stdio": null`.
- You may also use dictionary syntax: `"stdio": { "stdin": null, "stdout": null, "stderr": "/tmp/my.log" }`.

<sup>1</sup> On Windows debuggee is always launched in a new window, however stdio streams may still be redirected
as described above.<br>
<sup>2</sup> Use `tty` command inside a terminal window to find out its TTY device path.

## Attaching

Flow during the launch sequence attach sequence:
1. Debug session is created.
2. The `initCommands` sequence is executed.
3. Debug target is created using launch configurtion prameters  (`program`).
4. Breakpoints are set.
5. The `preRunCommands` sequence is executed.  These commands may alter debug target configuration.
6. The debugger attaches to the specified debuggee process.
7. The `postRunCommands` sequence is executed.

At the end of the debug session `exitCommands` sequence is executed.

Note that attaching to a running process may be [restricted](https://en.wikipedia.org/wiki/Ptrace#Support)
on some systems.  You may need to adjust system configuration to enable it.

|parameter          |type    |req |         |
|-------------------|--------|:--:|---------|
|**name**           |string  |Y| Launch configuration name.
|**type**           |string  |Y| Set to `lldb`.
|**request**        |string  |Y| Set to `attach`.
|**program**        |string  |Y| Path to debuggee executable.
|**pid**            |number  | | Process id to attach to.  **pid** may be omitted, in which case debugger will attempt to locate an already running instance of the program. You may also put `${command:pickProcess}` or `${command:pickMyProcess}` here to choose a process interactively.
|**stopOnEntry**    |boolean | | Whether to stop the debuggee immediately after attaching.
|**waitFor**        |boolean | | Wait for the process to launch.
|**initCommands**   |[string]| | LLDB commands executed upon debugger startup.
|**preRunCommands** |[string]| | LLDB commands executed just before attaching to the debuggee.
|**postRunCommands**|[string]| | LLDB commands executed just after attaching to the debuggee.
|**exitCommands**   |[string]| | LLDB commands executed at the end of debugging session.
|**expressions**    |string| | The default expression evaluator type: `simple`, `python` or `native`.  See [Expressions](#expressions).
|**sourceMap**      |dictionary| | See [Source Path Remapping](#source-path-remapping).
|**sourceLanguages**| A list of source languages used in the program.  This is used to enable language-specific debugger features.

## Custom Launch

The custom launch method puts you in complete control of how debuggee process is created.  This
happens in these steps:

1. The `targetCreateCommands` sequence is executed.  After this step a valid debug target is expected to exist.
2. The debugger inserts breakpoints.
3. The `processCreateCommands` sequence is executed.  After this step a valid debuggee process is expected to exist.
4. The debugger reports current state of the debuggee to VSCode.

|parameter          |type    |req |         |
|-------------------|--------|:--:|---------|
|**name**           |string  |Y| Launch configuration name.
|**type**           |string  |Y| Set to `lldb`.
|**request**        |string  |Y| Set to `custom`.
|**targetCreateCommands**  |[string]| | Commands that create the debug target.
|**processCreateCommands** |[string]| | Commands that create the debuggee process.
|**exitCommands**   |[string]| | LLDB commands executed at the end of debugging session.
|**expressions**    |string| | The default expression evaluator type: `simple`, `python` or `native`.  See [Expressions](#expressions).
|**sourceMap**      |dictionary| | See [Source Path Remapping](#source-path-remapping).
|**sourceLanguages**| A list of source languages used in the program.  This is used to enable language-specific debugger features.

## Remote debugging

For general information on remote debugging please see [LLDB Remote Debugging Guide](http://lldb.llvm.org/remote.html).

### Connecting to lldb-server agent
- Run `lldb-server platform --server --listen *:<port>` on the remote machine.
- Create launch configuration similar to this:
```javascript
{
    "name": "Remote launch",
    "type": "lldb",
    "request": "launch",
    "program": "${workspaceFolder}/build/debuggee", // Local path.
    "initCommands": [
        "platform select <platform>",
        "platform connect connect://<remote_host>:<port>"
    ],
}
```
See `platform list` for a list of available remote platform plugins.

- Start debugging as usual.  The executable identified by the `program` property will
be automatically copied to `lldb-server`'s current directory on the remote machine.
If you require additional configuration of the remote system, you may use `preRunCommands` sequence
to execute commands such as `platform mkdir`, `platform put-file`, `platform shell`, etc.
(See `help platform` for a list of available platform commands).

### Connecting to a gdbserver-style agent
(This includes not just gdbserver itself, but also environments that implement the gdbserver protocol,
 such as [OpenOCD](http://openocd.org/), [QEMU](https://www.qemu.org/), [rr](https://rr-project.org/), and others)

- Start remote agent. For example, run `gdbserver *:<port> <debuggee> <debuggee args>` on the remote machine.
- Create a custom launch configuration:
```javascript
{
    "name": "Remote attach",
    "type": "lldb",
    "request": "custom",
    "targetCreateCommands": ["target create ${workspaceFolder}/build/debuggee"],
    "processCreateCommands": ["gdb-remote <remote_host>:<port>"]
}
```
- Start debugging.

Please note that depending on protocol features implemented by the remote stub, there may be more setup needed.
For example, in the case of "bare-metal" debugging (OpenOCD), the debugger may not be aware of memory locations
of the debuggee modules; you may need to specify this manually:
```
target modules load --file ${workspaceFolder}/build/debuggee -s <base load address>
```

## Loading a Core Dump
Use custom launch with `target crate -c <core path>` command:
```javascript
{
    "name": "Core dump",
    "type": "lldb",
    "request": "custom",
    "targetCreateCommands": ["target create -c ${workspaceFolder}/core"],
}
```

## Source Path Remapping
Source path remapping is helpful in cases when program's source code is located in a different
directory then it was in during the build (for example, if a build server was used).

A source map consists of pairs of "from" and "to" path prefixes.  When the debugger encounters a source
file path beginning with one of the "from" prefixes, it will substitute the corresponding "to" prefix
instead.  Example:
```javascript
    "sourceMap": { "/build/time/source/path" : "/current/source/path" }
```

## Parameterized Launch Configurations
Sometimes you'll find yourself adding the same parameters (e.g. a path of a dataset directory)
to multiple launch configurations over and over again.  CodeLLDB provides a feature to help with
configuration management in such cases: you may put common configuration values in `lldb.dbgconfig`
section of the workspace configuration, then reference them using `${dbgconfig:variable}` syntax
in debug launch configurations:

```javascript
// settings.json
    ...
    "lldb.dbgconfig":
    {
        "dateset": "dataset1",
        "datadir": "${env:HOME}/mydata/${dbgconfig:dataset}" // "dbgconfig" properties may reference each other,
                                                             // as long as there is no recursion.
    }

// launch.json
    ...
    {
        "name": "Debug program",
        "type": "lldb",
        "program": "${workspaceFolder}/build/bin/program",
        "cwd": "${dbgconfig:datadir}" // will be expanded to "/home/user/mydata/dataset1"
    }
```

# Debugger Features

## Commands

|                                 |                                                         |
|---------------------------------|---------------------------------------------------------|
|**Show Disassembly...**         |Choose when the disassembly view is shown. See [Disassembly View](#disassembly-view).
|**Toggle Disassembly**           |Choose when the disassembly view is shown. See [Disassembly View](#disassembly-view).
|**Display Format...**           |Choose default variable display format. See [Formatting](#formatting).
|**Toggle Numeric Pointer Values**|Choose whether to display the pointee's value rather than numeric value of the pointer itself. See [Pointers](#pointers).
|**Display Options...**           |Interactive configuration of the above display options.
|**Run Diagnostics**              |Run diagnostic on LLDB, to make sure it can be used with this extension.  The command is executed automatically the first time when CodeLLDB is used.
|**Generate launch configurations from Cargo.toml**|Generate all possible launch configurations (binaries, examples, unit tests) for the current Rust project.  The resulting list will be opened in a new text editor, from which you can copy/paste the desired sections into `launch.json`.|


## Regex Breakpoints
Function breakpoints prefixed with '`/re `', are interpreted as regular expressions.
This causes a breakpoint to be set in every function matching the expression.
The list of created breakpoint locations may be examined using `break list` command.

## Conditional Breakpoints
You may use any of the supported expression [syntaxes](#expressions) to create breakpoint conditions.
When a breakpoint condition evaluates to False, the breakpoint will not be stopped at.
Any other value (or expression evaluation error) will cause the debugger to stop.

## Hit conditions (native adapter only)
Syntax:
```
    operator :: = '<' | '<=' | '=' | '>=' | '>' | '%'
    hit_condition ::= operator number
```

The `'%'` operator causes a stop after every `number` of breakpoint hits.

## Logpoints
Expressions embedded in log messages via curly brackets may use any of the supported expression [syntaxes](#expressions).

## Disassembly View
When execution steps into code for which debug info is not available, CodeLLDB will automatically
switch to disassembly view.  This behavior may be controlled using **Show Disassembly**
and **Toggle Disassembly** commands.  The former allows to choose between `never`,
`auto` (the default) and `always`, the latter toggles between `auto` and `always`.

While is disassembly view, 'step over' and 'step into' debug actions will perform instruction-level
stepping rather than source-level stepping.

![disassembly view](images/disasm.png)

## Formatting
You may change the default display format of evaluation results using the `Display Format` command.

When evaluating expressions in Debug Console or in Watch panel, you may control formatting of
individual expressions by adding one of the suffixes listed below.  For example evaluation of `var,x`
will display the value of `var` formatted as hex.

|suffix |format |
|:-----:|-------|
|**x**  | Hex
|**o**  | Octal
|**d**  | Decimal
|**u**  | Unsigned decimal
|**b**  | Binary
|**f**  | Float (reinterprets bits, no casting is done)
|**p**  | Pointer
|**s**  | C string
|**y**  | Bytes
|**Y**  | Bytes with ASCII
|**[\<num\>]**| Reinterpret as an array of \<num\> elements

### Pointers

When displaying pointer and reference variables, CodeLLDB will prefer to display the
value of the object pointed to.  If you would like to see the raw address value,
you may toggle this behavior using **Toggle Numeric Pointer Values** command.
Another way to display raw pointer address is to add the pointer variable to Watch panel and specify
an explicit format, as described in the previous section.

## LLDB Commands
To access LLDB features not exposed via the VS Code UI, you may enter
[LLDB commands](http://lldb.llvm.org/tutorial.html) directly into the Debug Console.

If you would like to evaluate an expression instead, prefix it with '`?`', e.g. `?a+b`.

## Expressions

CodeLLDB implements three expression evaluator types: "simple", "python" and "native".  These are used
wherever user-entered expression needs to be evaluated: in "Watch" panel, in the Debug Console (for input
prefixed with `?`) and in breakpoint conditions.<br>
By default, "simple" is assumed, however you may change this using the [expressions](#launching) launch
configuration property.  The default type may also be overridden on a per-expression basis by using a prefix.

### Simple expressions
Prefix: `/se `<br>
Simple expressions consist of debuggee's variables (local or static), Python operators, as well as
operator keywords `and`, `or`, `not`.  No other Python keywords are allowed.
The values of debuggee variables are obtained through [LLDB data formatters](https://lldb.llvm.org/varformats.html),
thus if you have formatters installed for specific library types, they will work as expected.
For example, things like indexing an `std::vector` with an integer, or comparing `std::string` to
a string literal should just work. Variables, whose names are not valid Python identifiers may be accessed by escaping them with `${`...`}`.

### Python expressions
Prefix: `/py `<br>
Python expressions use normal Python syntax.  In addition to that, any identifier prefixed with `$`
(or enclosed in `${`...`}`), will be replaced with the value of the corresponding debuggee
variable.  Such values may be mixed with regular Python variables.  For example, `/py [math.sqrt(x) for x in $arr]`
will evaluate to a list containing square roots of  values contained in debuggee's array `arr`.

### Native expressions
Prefix: `/nat `<br>
These use LLDB built-in expression evaluators.  The specifics depend on source language of the
current debug target (e.g. C, C++ or Swift).<br>
For example, the C++ expression evaluator offers many powerful features including interactive definition
of new data types, instantiation of C++ classes, invocation of functions and class methods, and more.

Note, however, that native evaluators ignore data formatters and operate on "raw" data structures,
thus they are often not as convenient as "simple" or "python" expressions.

## Debugger API

CodeLLDB provides a Python API via the `debugger` module (which is auto-imported into
debugger's main script context).

|Function                           |Description|
|-----------------------------------|------------
|**evaluate(expression: `str`) -> `Value`**| Allows dynamic evaluation of [simple expressions](#simple-expressions). The returned `Value` type is a proxy wrapper around `lldb.SBValue`,<br> which overloads most of Python's operators, so that arithmetic expressions work as one would expect.
|**unwrap(obj: `Value`) -> `lldb.SBValue`**| Extracts [`lldb.SBValue`](https://lldb.llvm.org/python_reference/lldb.SBValue-class.html) from `Value`.
|**wrap(obj: `lldb.SBValue`) -> `Value`**| Wraps [`lldb.SBValue`](https://lldb.llvm.org/python_reference/lldb.SBValue-class.html) in a `Value` object.
|**display_html(<br>&nbsp;&nbsp;&nbsp;&nbsp;html: `str`, title: `str` = None,<br>&nbsp;&nbsp;&nbsp;&nbsp;position: `int` = None, reveal: `bool` = False)**|Displays content in a VSCode Webview panel:<li>html: HTML markup to display.<li> title: Title of the panel.  Defaults to name of the current launch configuration.<li>position: Position (column) of the panel.  The allowed range is 1 through 3.<li>reveal: Whether to reveal a panel, if one already exists.


# Alternate LLDB backends
*(native adapter only)*<br>
CodeLLDB can use external LLDB backends instead of the bundled one.  For example, when debugging
Swift programs, one might want to use a custom LLDB instance that has Swift extensions built in.<br>
In order to use alternate backend, you will need to provide location of the corresponding liblldb&#46;so/.dylib/.dll
dynamic library via the **lldb.library** configuration setting. Alternatively, it is also possible to provide name of the main LLDB executable (via **lldb.executable**), in which case CodeLLDB will attempt to locate the library automatically.

# Rust Language Support

CodeLLDB natively supports visualization of most common Rust data types:
- Built-in types: tuples, enums, arrays, array and string slices.
- Standard library types: Vec, String, CString, OSString.

To enable this feature, add `"sourceLanguages": ["rust"]` into your launch configuration.

Note: There is a known incompatibility of debug info emitted by `rustc` and LLDB 3.8:
you won't be able to step through code or inspect variables if you have this version.
The workaround is to use either LLDB 3.7 or 3.9.  On macOS, LLDB shipped with Xcode 8 is known to
have this problem fixed.

![source](images/source.png)

## Cargo support

Several Rust users had pointed out that debugging tests and benches in Cargo-based projects is somewhat
difficult since names of the output test/bench binary generated by Cargo is not deterministic.
To cope with this problem, CodeLLDB can now query Cargo for names of the compilation artifacts.  In order
to use this feature, replace `program` property in your launch configuration with `cargo`:
```javascript
{
    "type": "lldb",
    "request": "launch",
    "cargo": {
        "args": ["test", "--no-run", "--lib"], // Cargo command line to build the debug target
        // "args": ["build", "--bin=foo"] is another possibility
        "filter": { // Filter applied to compilation artifacts (optional)
            "name": "mylib",
            "kind": "lib"
        }
    }
}
```
Try to be as specific as possible when specifying the build target, because if there's more than one
binary output, CodeLLDB won't know which one you want to debug!

Normally, Cargo output will be used to set the `program` property (but only if it isn't defined).
However, in order to support custom launch and other odd-ball scenarios, there is also
a substitution variable, which expands to the same thing: `${cargo:program}`.

CodeLLDB will also use `Cargo.toml` in the workspace root to generate initial debug
configurations (if there is no `launch.json` in the workspace).

# Workspace Configuration

## General
|                       |                                                         |
|-----------------------|---------------------------------------------------------|
|**lldb.dbgconfig**     |See [Parameterized Launch Configurations](#parameterized-launch-configurations).
|**lldb.evaluationTimeout**|Timeout for expression evaluation, in seconds (default=5s).
|**lldb.displayFormat**|The default format for variable and expression values.
|**lldb.showDisassembly**|When to show disassembly:<li>auto - only when source is not available.,<li>never - never show.,<li>always - always show, even if source is available.
|**lldb.dereferencePointers**|Whether to show a summary of the pointee, or a numeriric value for pointers.
|**lldb.suppressMissingSourceFiles**|Suppress VSCode's messages about missing source files (when debug info refers to files not present on the local machine).

## Advanced
|                       |                                                         |
|-----------------------|---------------------------------------------------------|
|**lldb.adapterType**   |Type of debug adapter to use:<li>classic - a Python-based debug adapter running in externally provided LLDB,<li>bundled - a Python-based debug adapter running in LLDB provided by this extension (based on LLDB 8.0),<li>native - native debug adapter (based on libLLDB 8.0).<br>The last two options will require one-time download of platform-specific binaries.
|**lldb.executable**    |Which LLDB executable to use. (default="lldb")
|**lldb.library**       |Which LLDB library to use (native adapter only). This can be either a file path (recommended) or a directory, in which case platform-specific heuristics will be used to locate the actual library file.
|**lldb.adapterEnv**|Environment variables to pass to the debug adapter.
|**lldb.verboseLogging**|Enables verbose logging.  The log can be viewed in the "LLDB" output panel.

## Default launch configuration settings
|                       |                                                         |
|-----------------------|---------------------------------------------------------|
|**lldb.launch.initCommands** |Commands executed *before* initCommands in individual launch configurations.
|**lldb.launch.preRunCommands** |Commands executed *before* preRunCommands in individual launch configurations.
|**lldb.launch.exitCommands** |Commands executed *after* exitCommands in individual launch configurations.
|**lldb.launch.env** |Additional environment variables that will be merged with 'env' of individual launch configurations.
|**lldb.launch.cwd** |Default program working directory.
|**lldb.launch.stdio** |Default stdio destination.
|**lldb.launch.terminal** |Default terminal type.
|**lldb.launch.sourceMap** |Additional entries that will be merged with 'sourceMap's of individual launch configurations.
|**lldb.launch.sourceLanguages**| A list of source languages used in the program.  This is used to enable language-specific debugger features.
