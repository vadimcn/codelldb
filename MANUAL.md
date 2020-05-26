
# Table of Contents

- How to:
    - [Starting a New Debug Session](#starting-a-new-debug-session)
        - [Launching a New Process](#launching-a-new-process)
            - [Stdio Redirection](#stdio-redirection)
        - [Attaching to an Existing Process](#attaching-to-a-running-process)
        - [Custom Launch](#custom-launch)
    - [Starting Debug Session Outside of VSCode](#starting-debug-session-outside-of-vscode)
    - [Remote Debugging](#remote-debugging)
    - [Reverse Debugging](#reverse-debugging) (experimental)
    - [Inspecting a Core Dump](#inspecting-a-core-dump)
    - [Source Path Remapping](#source-path-remapping)
    - [Parameterized Launch Configurations](#parameterized-launch-configurations)
- [Debugger Features](#debugger-features)
    - [Commands](#commands)
    - [Regex Breakpoints](#regex-breakpoints)
    - [Conditional Breakpoints](#conditional-breakpoints)
    - [Data Breakpoints](#data-breakpoints)
    - [Disassembly View](#disassembly-view)
    - [Formatting](#formatting)
        - [Pointers](#pointers)
    - [Expressions](#expressions)
    - [Debugger API](#debugger-api)
- [Adapter types](#adapter-types)
- [Alternate LLDB backends](#alternate-lldb-backends)
- [Rust Language Support](#rust-language-support)
- [Workspace Configuration](#workspace-configuration)

# Starting a New Debug Session

To start a debug session you will need to create a [launch configuration](https://code.visualstudio.com/Docs/editor/debugging#_launch-configurations) for your program.  The `request` property of the configuration chooses how it will be done:

## Launching a New Process

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
|**relativePathBase**|string| | Base directory used for resolution of relative source paths.  Defaults to "${workspaceFolder}".
|**sourceLanguages**| A list of source languages used in the program.  This is used to enable language-specific debugger features.

Flow during the launch sequence:
1. Debug session is created.
2. The `initCommands` sequence is executed.
3. Debug target is created using launch configuration parameters (`program`, `args`, `env`, `cwd`, `stdio`).
4. Breakpoints are set.
5. The `preRunCommands` sequence is executed.  These commands may alter debug target configuration.
6. Debuggee is launched.
7. The `postRunCommands` sequence is executed.

At the end of the debug session `exitCommands` sequence is executed.

### Stdio Redirection
The **stdio** property is a list of redirection targets for each of the debuggee's stdio streams:
- `null` value will cause redirect to the default debug session terminal (as specified by the **terminal** launch property),
- `"/some/path"` will cause the stream to be redirected to the specified file, pipe or a TTY device<sup>*</sup>,
- if you provide less than 3 values, the list will be padded to 3 entries using the last provided value,
- you may specify more than three values, in which case additional file descriptors will be created (4, 5, etc.)

Examples:
- `"stdio": [null, "log.txt", null]` - connect stdin and stderr to the default terminal, while sending
stdout to "log.txt",
- `"stdio": ["input.txt", "log.txt"]` - connect stdin to "input.txt", while sending both stdout and stderr to "log.txt",
- `"stdio": null` - connect all three streams to the default terminal.

<sup>*</sup> Run `tty` command in a terminal window to find out the TTY device name.

## Attaching to a Running Process

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
|**relativePathBase**|string| | Base directory used for resolution of relative source paths.  Defaults to "${workspaceFolder}".
|**sourceLanguages**| A list of source languages used in the program.  This is used to enable language-specific debugger features.

Flow during the attach sequence:
1. Debug session is created.
2. The `initCommands` sequence is executed.
3. Debug target is created using launch configuration parameters  (`program`).
4. Breakpoints are set.
5. The `preRunCommands` sequence is executed.  These commands may alter debug target configuration.
6. The debugger attaches to the specified debuggee process.
7. The `postRunCommands` sequence is executed.

At the end of the debug session `exitCommands` sequence is executed.

Note that attaching to a running process may be [restricted](https://en.wikipedia.org/wiki/Ptrace#Support)
on some systems.  You may need to adjust system configuration to enable it.

## Custom Launch

The custom launch method allows user to fully specify how the debug session is initiated.  The flow of custom launch is as follows:

1. The `targetCreateCommands` command sequence is executed.  It is expected that a debug target will have been created upon completion.
2. Debugger inserts source breakpoints.
3. The `processCreateCommands` command sequence is executed.  This sequence is expected to create the debuggee process.
4. Debugger reports current state of the debuggee process to VSCode and starts accepting user commands.

|parameter          |type    |req |         |
|-------------------|--------|:--:|---------|
|**name**           |string  |Y| Launch configuration name.
|**type**           |string  |Y| Set to `lldb`.
|**request**        |string  |Y| Set to `custom`.
|**initCommands**   |[string]| | LLDB commands executed upon debugger startup.
|**targetCreateCommands**  |[string]| | Commands that create the debug target.
|**processCreateCommands** |[string]| | Commands that create the debuggee process.
|**exitCommands**   |[string]| | LLDB commands executed at the end of debugging session.
|**expressions**    |string| | The default expression evaluator type: `simple`, `python` or `native`.  See [Expressions](#expressions).
|**sourceMap**      |dictionary| | See [Source Path Remapping](#source-path-remapping).
|**relativePathBase**|string| | Base directory used for resolution of relative source paths.  Defaults to "${workspaceFolder}".
|**sourceLanguages**| A list of source languages used in the program.  This is used to enable language-specific debugger features.
|**reverseDebugging**|bool| | Enable [reverse debugging](#reverse-debugging).


## Starting Debug Session Outside of VSCode

Debug sessions may also be started outside of VSCode by invoking a specially formatted URI:

- **`vscode://vadimcn.vscode-lldb/launch?name=<configuration name>,[folder=<path>]`**</br>
  This will start a new debug session using the named launch configuration.  The optional `folder` parameter specifies
  workspace folder where the launch configuration is defined. If missing, all folders in the current workspace will be searched.<br>
  Example: `code --open-url "vscode://vadimcn.vscode-lldb/launch?name=Debug My Project`
- **`vscode://vadimcn.vscode-lldb/launch/command?<env1>=<val1>&<env2>=<val2>&<command-line>`**</br>
  The \<command-line\> will be split into program name and arguments array using the usual shell command-line parsing rules.<br>
  Example: `code --open-url "vscode://vadimcn.vscode-lldb/launch/command?/path/filename arg1 \"arg 2\" arg3"`
- **`vscode://vadimcn.vscode-lldb/launch/config?<json>`**</br>
  This endpoint accepts a <a href="https://json5.org/">JSON5</a> snippet matching one of the above debug session initiation methods.
  The `type` and `request` attributes may be omitted, and will default to "lldb" and "launch" respectively.<br>
  Example: `code --open-url "vscode://vadimcn.vscode-lldb/launch/config?{program:'/path/filename', args:['arg1','arg 2','arg3']}"`

### Applications

- Attach debugger to the current process:
    ```C
    char command[256];
    snprintf(command, sizeof(command), "code --open-url \"vscode://vadimcn.vscode-lldb/launch/config?{request:'attach',pid:%d}\"", getpid());
    system(command);
    sleep(1); // Wait for the debugger to attach
    ```

- Same in Rust (did you ever want to debug a build script?):
    ```Rust
    let url = format!("vscode://vadimcn.vscode-lldb/launch/config?{{request:'attach',pid:{}}}", std::process::id());
    std::process::Command::new("code").arg("--open-url").arg(url).output().unwrap();
    std::thread::sleep_ms(1000);
    ```

- Have Rust unit tests executed under debugger:<br>
    - Create `.cargo` directory in your project folder containing these two files:
        - `config` [(see also)](https://doc.rust-lang.org/cargo/reference/config.html)
            ```TOML
            [target.<current-target-triple>]
            runner = ".cargo/codelldb"
            ```
        - `codelldb`
            ```sh
            #! /bin/bash
            code --open-url "vscode://vadimcn.vscode-lldb/launch/command?LD_LIBRARY_PATH=$LD_LIBRARY_PATH&$*"
            ```
    - `chmod +x .cargo/codelldb`
    - Execute tests as normal.

### Notes
- All URIs above are subject to normal [URI encoding rules](https://en.wikipedia.org/wiki/Percent-encoding), therefore all '%' characters must be escaped as '%25'.   A more rigorous launcher script would have done that :)<br>
- VSCode URIs may also be invoked using OS-specific tools:
  - Linux: `xdg-open <uri>`
  - MacOS: `open <uri>`
  - Windows: `start <uri>`

## Remote debugging

For general information on remote debugging please see [LLDB Remote Debugging Guide](http://lldb.llvm.org/remote.html).

### Connecting to lldb-server agent
- Run `lldb-server platform --server --listen *:<port>` on the remote machine.
- Create launch configuration similar to the one below.
- Start debugging as usual.  The executable identified by the `program` property will
be automatically copied to `lldb-server`'s current directory on the remote machine.
If you require additional configuration of the remote system, you may use `preRunCommands` sequence
to execute commands such as `platform mkdir`, `platform put-file`, `platform shell`, etc.
(See `help platform` for a list of available platform commands).

```javascript
{
    "name": "Remote launch",
    "type": "lldb",
    "request": "launch",
    "program": "${workspaceFolder}/build/debuggee", // Local path.
    "initCommands": [
        "platform select <platform>", // Execute `platform list` for a list of available remote platform plugins.
        "platform connect connect://<remote_host>:<port>",
        "settings set target.inherit-env false", // See the note below.
    ],
    "env": {
        "PATH": "...", // See the note below.
    }
}
```
Note: By default, debuggee will inherit environment from the debugger.  However, this environment  will be of your
**local** machine.  In most cases these values will not be suiteble on the remote
system, so you should consider disabling environment inheritance with `settings set target.inherit-env false` and
initializing them as appropriate, starting with `PATH`.

### Connecting to a gdbserver-style agent
This includes not just gdbserver itself, but also execution environments that implement the gdbserver protocol,
such as [OpenOCD](http://openocd.org/), [QEMU](https://www.qemu.org/), [rr](https://rr-project.org/), and others.

- Start remote agent. For example, run `gdbserver *:<port> <debuggee> <debuggee args>` on the remote machine.
- Create a custom launch configuration.
- Start debugging.
```javascript
{
    "name": "Remote attach",
    "type": "lldb",
    "request": "custom",
    "targetCreateCommands": ["target create ${workspaceFolder}/build/debuggee"],
    "processCreateCommands": ["gdb-remote <remote_host>:<port>"]
}
```


Please note that depending on protocol features implemented by the remote stub, there may be more setup needed.
For example, in the case of "bare-metal" debugging (OpenOCD), the debugger may not be aware of memory locations
of the debuggee modules; you may need to specify this manually:
```
target modules load --file ${workspaceFolder}/build/debuggee -s <base load address>`
```

## Reverse Debugging

Also known as [Time travel debugging](https://en.wikipedia.org/wiki/Time_travel_debugging).  Provided you use a debugging backend that supports
[these commands](https://sourceware.org/gdb/onlinedocs/gdb/Packets.html#bc), CodeLLDB be used to control reverse execution and stepping.

As of this writing, the only backend known to work is [Mozilla's rr](https://rr-project.org/).  The minimum supported version is 5.3.0.

There are others mentioned [here](http://www.sourceware.org/gdb/news/reversible.html) and [here](https://github.com/mozilla/rr/wiki/Related-work).
[QEMU](https://www.qemu.org/) reportedly [supports record/replay](https://github.com/qemu/qemu/blob/master/docs/replay.txt) in full system emulation mode.
If you get any of them to work, please let me know!

### Example: (using rr)
Record execution trace:
```sh
rr record <debuggee> <arg1> ...
```

Replay execution:
```sh
rr replay -s <port>
```

Launch config:
```javascript
{
    "name": "Replay",
    "type": "lldb",
    "request": "custom",
    "targetCreateCommands": ["target create ${workspaceFolder}/build/debuggee"],
    "processCreateCommands": ["gdb-remote 127.0.0.1:<port>"],
    "reverseDebugging": true
}
```

## Inspecting a Core Dump
Use custom launch with `target create -c <core path>` command:
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
directory then it was in at build time (for example, if a build server was used).

A source map consists of pairs of "from" and "to" path prefixes.  When the debugger encounters a source
file path beginning with one of the "from" prefixes, it will substitute the corresponding "to" prefix
instead.  Example:
```javascript
    "sourceMap": { "/build/time/source/path" : "/current/source/path" }
```

## Parameterized Launch Configurations
Sometimes you'll find yourself adding the same parameters (e.g. a path of a dataset directory)
to multiple launch configurations over and over again.  CodeLLDB can help with configuration management
in such cases: you can place common configuration values into `lldb.dbgconfig` section of the workspace configuration,
then reference via `${dbgconfig:variable}` in launch configurations.<br>
Example:

```javascript
// settings.json
    ...
    "lldb.dbgconfig":
    {
        "dataset": "dataset1",
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
|**Show Disassembly...**          |Choose when disassembly is shown. See [Disassembly View](#disassembly-view).
|**Toggle Disassembly**           |Choose when disassembly is shown. See [Disassembly View](#disassembly-view).
|**Display Format...**            |Choose the default variable display format. See [Formatting](#formatting).
|**Toggle Pointee Summaries**     |Choose whether to display pointee's summaries rather than the numeric value of the pointer itself. See [Pointers](#pointers).
|**Display Options...**           |Interactive configuration of the above display options.
|**Attach to Process...**         |Choose a process to attach to from the list of currently running processes.
|**Use Alternate Backend...**     |Choose alternate LLDB instance to be used instead of the bundled one. See [Alternate LLDB backends](#alternate-lldb-backends)
|**Run Diagnostics**              |Run diagnostic test to make sure that the debugger is functional.
|**Generate launch configurations from Cargo.toml**|Generate all possible launch configurations (binaries, examples, unit tests) for the current Rust project.  The resulting list will be opened in a new text editor, from which you can copy/paste the desired sections into `launch.json`.|


## Regex Breakpoints
Function breakpoints prefixed with '`/re `', are interpreted as regular expressions.
This causes a breakpoint to be set in every function matching the expression.
The list of created breakpoint locations may be examined using `break list` command.

## Conditional Breakpoints
You may use any of the supported expression [syntaxes](#expressions) to create breakpoint conditions.
When a breakpoint condition evaluates to False, the breakpoint will not be stopped at.
Any other value (or expression evaluation error) will cause the debugger to stop.

## Data Breakpoints
Data breakpoints (or "watchpoints" in LLDB terms) allow monitoring memory location for changes.  You can create data
breakpoints by choosing "Break When Value Changes" from context menu in the Variables panel. (To access advanced features,
such as breaking on memory reads, use LLDB `watch` command).

Note that data breakpoints require hardware support, and, as such, may come with restrictions, depending on CPU platform and OS support.
For example, on x86_64 the restrictions are as follows:
- The monitored memory region must be 1, 2, 4 or 8 bytes in size.
- There may be at most 4 data watchpoints.


## Hit conditions
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
you may toggle this behavior using **Toggle Pointee Summaries** command.
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
By default, "simple" is assumed, however you may change this using the [expressions](#launching-a-new-process) launch
configuration property.  The default type may also be overridden on a per-expression basis using a prefix.

### Simple expressions
Prefix: `/se `<br>
Simple expressions are a subset of Python expressions consisting of identifiers, arithmetic operators, indexing,
and logical operators `and`, `or`, `not`.  All other identifiers (including Python keywords) will evaluate to the value
of the corresponding debuggee variable in the currently selected stack frame.  These values are obtained after applying
[LLDB data formatters](https://lldb.llvm.org/varformats.html), so you will get the the "formatted" view of variables.
For example, things like indexing an `std::vector` with an integer or comparing `std::string` to a string literal should just work.
Variables, whose names are not valid Python identifiers may be accessed by escaping them with `${`...`}`.

### Python expressions
Prefix: `/py `<br>
Python expressions use normal Python syntax.  In addition to that, any identifier prefixed by `$`
(or enclosed in `${`...`}`), will be replaced with the value of the corresponding debuggee
variable.  Such values may be mixed with regular Python variables.  For example, `/py [math.sqrt(x) for x in $arr]`
will evaluate to a list containing square roots of the values contained in array variable `arr`.

### Native expressions
Prefix: `/nat `<br>
Native expressions use LLDB's built-in expression evaluators.  The specifics depend on source language of the
current debug target (e.g. C, C++ or Swift).<br>
For example, the C++ expression evaluator offers many powerful features including interactive definition
of new data types, instantiation of C++ classes, invocation of functions and class methods, and more.

Note, however, that native evaluators ignore data formatters and operate on "raw" data structures,
thus they are often not as convenient as "simple" or "python" expressions.

## Debugger API

CodeLLDB provides Python API via the `debugger` module (which is auto-imported into
debugger's main script context).

|Function                           |Description|
|-----------------------------------|------------
|**evaluate(expression: `str`) -> `Value`**| Allows dynamic evaluation of [simple expressions](#simple-expressions). The returned `Value` object is a proxy wrapper around [`lldb.SBValue`](https://lldb.llvm.org/python_reference/lldb.SBValue-class.html),<br> which implements most Python operators over the underlying value.
|**unwrap(obj: `Value`) -> `lldb.SBValue`**| Extracts an [`lldb.SBValue`](https://lldb.llvm.org/python_reference/lldb.SBValue-class.html) from `Value`.
|**wrap(obj: `lldb.SBValue`) -> `Value`**| Wraps [`lldb.SBValue`](https://lldb.llvm.org/python_reference/lldb.SBValue-class.html) in a `Value` object.
|**display_html(<br>&nbsp;&nbsp;&nbsp;&nbsp;html: `str`, title: `str` = None,<br>&nbsp;&nbsp;&nbsp;&nbsp;position: `int` = None, reveal: `bool` = False)**|Displays content in a VSCode Webview panel:<li>html: HTML markup to display.<li> title: Title of the panel.  Defaults to name of the current launch configuration.<li>position: Position (column) of the panel.  The allowed range is 1 through 3.<li>reveal: Whether to reveal a panel, if one already exists.

# Alternate LLDB backends
CodeLLDB can use external LLDB backends instead of the bundled one.  For example, when debugging
Swift programs, one might want to use a custom LLDB instance that has Swift extensions built in.<br>
In order to use alternate backend, you will need to provide location of the corresponding liblldb&#46;so/.dylib/.dll
dynamic library via the **lldb.library** configuration setting.
Since locating liblldb is not always trivial, CodeLLDB provides the **Use Alternate Backend...** command to assist with this task.
You will be prompted to enter the file name of the main LLDB executable, which CodeLLDB will then use to find the corresponding library.


# Rust Language Support

CodeLLDB natively supports visualization of most common Rust data types:
- Built-in types: tuples, enums, arrays, array and string slices.
- Standard library types: `Vec`, `String`, `CString`, `OSString`, `Path`, `Cell`, `Rc`, `Arc` and more.

To enable this feature, add `"sourceLanguages": ["rust"]` into your launch configuration.

![source](images/source.png)

## Cargo support

Several Rust users had pointed out that debugging tests and benches in Cargo-based projects is somewhat
difficult since names of the output test/bench binary generated by Cargo is not deterministic.
To cope with this problem, CodeLLDB can query Cargo for a list of its compilation outputs.  In order
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
Try to be as specific as possible when specifying the build target, because if there is more than one
binary output, CodeLLDB won't know which one you want it to debug!

Normally, Cargo output will be used to set the `program` property (but only if it isn't defined).
However, in order to support custom launch and other oddball scenarios, there is also
a substitution variable, which expands to the same thing: `${cargo:program}`.

CodeLLDB will also use `Cargo.toml` in the workspace root to generate initial debug
configurations when there is no existing `launch.json`.

# Workspace Configuration

## General
|                       |                                                         |
|-----------------------|---------------------------------------------------------|
|**lldb.dbgconfig**     |See [Parameterized Launch Configurations](#parameterized-launch-configurations).
|**lldb.evaluationTimeout**|Timeout for expression evaluation, in seconds (default=5s).
|**lldb.displayFormat**|The default format for variable and expression values.
|**lldb.showDisassembly**|When to show disassembly:<li>`auto` - only when source is not available.,<li>`never` - never show.,<li>`always` - always show, even if source is available.
|**lldb.dereferencePointers**|Whether to show a summary of the pointee, or a numeric value for pointers.
|**lldb.suppressMissingSourceFiles**|Suppress VSCode's messages about missing source files (when debug info refers to files not present on the local machine).
|**lldb.consoleMode**|Controls whether the debug console input is by default treated as debugger commands or as expressions to evaluate:<li>`commands` - treat debug console input as debugger commands.  In order to evaluate an expression, prefix it with '?' (question mark).",<li>`evaluate` - treat debug console input as expressions.  In order to execute a debugger command, prefix it with '`' (backtick).

## Advanced
|                       |                                                         |
|-----------------------|---------------------------------------------------------|
|**lldb.library**       |Which LLDB library to use. This can be either a file path (recommended) or a directory, in which case platform-specific heuristics will be used to locate the actual library file.
|**lldb.adapterEnv**|Environment variables to pass to the debug adapter.
|**lldb.verboseLogging**|Enables verbose logging.  The log can be viewed in the "LLDB" output panel.

## Default launch configuration settings
|                       |                                                         |
|-----------------------|---------------------------------------------------------|
|**lldb.launch.initCommands** |Commands executed *before* initCommands of individual launch configurations.
|**lldb.launch.preRunCommands** |Commands executed *before* preRunCommands of individual launch configurations.
|**lldb.launch.postRunCommands**|Commands executed *before* postRunCommands of individual launch configurations.
|**lldb.launch.exitCommands** |Commands executed *after* exitCommands of individual launch configurations.
|**lldb.launch.env** |Additional environment variables that will be merged with 'env' of individual launch configurations.
|**lldb.launch.cwd** |Default program working directory.
|**lldb.launch.stdio** |Default stdio destination.
|**lldb.launch.expressions** |Default expression evaluator.
|**lldb.launch.terminal** |Default terminal type.
|**lldb.launch.sourceMap** |Additional entries that will be merged with 'sourceMap's of individual launch configurations.
|**lldb.launch.relativePathBase**|string| | Default base directory used for resolution of relative source paths.  Defaults to "${workspaceFolder}".
|**lldb.launch.sourceLanguages**| A list of source languages used in the program.  This is used to enable language-specific debugger features.
