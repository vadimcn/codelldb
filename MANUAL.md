
# Table of Contents

- How to:
    - [Starting a New Debug Session](#starting-a-new-debug-session)
        - [Launching a New Process](#launching-a-new-process)
            - [Stdio Redirection](#stdio-redirection)
        - [Attaching to an Existing Process](#attaching-to-a-running-process)
    - [Debugging Externally Launched Code](#debugging-externally-launched-code)
        - [RPC Server](#rpc-server)
    - [Remote Debugging](#remote-debugging)
    - [Reverse Debugging](#reverse-debugging) (experimental)
    - [Inspecting a Core Dump](#inspecting-a-core-dump)
    - [Source Path Remapping](#source-path-remapping)
    - [Parameterized Launch Configurations](#parameterized-launch-configurations)
- [Debugger Features](#debugger-features)
    - [VSCode Commands](#vscode-commands)
    - [Debugger Commands](#debugger-commands)
    - [Debug Console](#debug-console)
    - [Regex Breakpoints](#regex-breakpoints)
    - [Conditional Breakpoints](#conditional-breakpoints)
    - [Data Breakpoints](#data-breakpoints)
    - [Disassembly View](#disassembly-view)
    - [Excluded Callers](#excluded-callers)
    - [Formatting](#formatting)
        - [Pointers](#pointers)
    - [Expressions](#expressions)
- [Python Scripting](#python-scripting)
    - [Debugger API](#debugger-api)
- [Alternate LLDB backends](#alternate-lldb-backends)
- [Rust Language Support](#rust-language-support)
- [Settings](#settings)
  - [Workspace Settings](#workspace-settings)
  - [Launch Configuration Settings](#launch-configurations-settings)
  - [LLDB Settings](#lldb-settings)
- [Workspace Configuration Reference](#workspace-configuration-reference)

# Starting a New Debug Session

To start a debugging session, you will need to create a [launch configuration](https://code.visualstudio.com/Docs/editor/debugging#_launch-configurations) for your program.   Here's a minimal one:

```javascript
{
    "name": "Launch",
    "type": "lldb",
    "request": "launch",
    "program": "${workspaceFolder}/<executable file>",
    "args": ["-arg1", "-arg2"],
}
```

 These attributes are common to all CodeLLDB launch configurations:

|attribute                |type  |         |
|-------------------------|------|---------|
|**name**                 |string| *Required.* Launch configuration name, as you want it to appear in the Run and Debug panel.
|**type**                 |string| *Required.* Set to `lldb`.
|**request**              |string| *Required.* Session initiation method:<br><li>`launch` to [create a new process](#launching-a-new-process),<br><li>`attach` to [attach to an already running process](#attaching-to-a-running-process).
|**initCommands**         |[string]| LLDB commands executed upon debugger startup.
|**targetCreateCommands**|[string]| LLDB commands executed to create debug target.
|**preRunCommands**       |[string]| LLDB commands executed just before launching of attaching to the debuggee.
|**processCreateCommands**|[string]| LLDB commands executed to create/attach the debuggee process.
|**postRunCommands**      |[string]| LLDB commands executed just after launching or attaching to the debuggee.
|**preTerminateCommands** |[string]| LLDB commands executed just before the debuggee is terminated or disconnected from.
|**exitCommands**         |[string]| LLDB commands executed at the end of the debugging session.
|**expressions**          |string| The default expression evaluator type: `simple`, `python` or `native`.  See [Expressions](#expressions).
|**sourceMap**            |dictionary| See [Source Path Remapping](#source-path-remapping).
|**relativePathBase**     |string | Base directory used for resolution of relative source paths.  Defaults to "${workspaceFolder}".
|**breakpointMode**       |enum | Specifies how source breakpoints should be set:<br><li>`path` - Resolve locations using full source file path (default).<li>`file` - Resolve locations using file name only.  This option may be useful in lieu of configuring `sourceMap`, however, note that breakpoints will be set in all files of the same name in the project.  For example, Rust projects often have lots of files named "mod.rs".
|**sourceLanguages**      |[string]| A list of source languages used in the program.  This is used to enable language-specific debugger features.
|**reverseDebugging**     |bool   | Enable [reverse debugging](#reverse-debugging).



## Launching a New Process

These attributes are applicable when the "launch" initiation method is selected:

|attribute          |type|         |
|-------------------|----|---------|
|**program**        |string| Path of the executable file.  *Required*, unless using `cargo` attribute.
|**cargo**          |string| See [Cargo support](#cargo-support).
|**args**           |string &#10072; [string]| Command line parameters.  If this is a string, it will be split using shell-like syntax.
|**cwd**            |string| Working directory.
|**env**            |dictionary| Environment variables to set in addition to the ones inherited from the parent process environment (unless LLDB's `target.inherit-env` setting has been set to `false`, in which case the initial process environment is empty).  You may refer to existing environment variables using `${env:NAME}` syntax.  For example, in order to alter the inherited `PATH` variable, you can do this: `"PATH":"${env:HOME}/bin:${env:PATH}"`.
|**envFile**        |string| Path of the file to read the environment variables from.  Note that `env` entries will override `envPath` entries.
|**stdio**          |string &#10072; [string] &#10072; dictionary| See [Stdio Redirection](#stdio-redirection).
|**terminal**       |string| Destination for debuggee's stdio streams: <ul><li>`console` for DEBUG CONSOLE</li><li>`integrated` (default) for VSCode integrated terminal</li><li>`external` for a new terminal window</li></ul>
|**stopOnEntry**    |boolean| Whether to stop debuggee immediately after launching.

Operations performed for launch:
- The `initCommands` sequence is executed.
- The [debug target object](https://lldb.llvm.org/python_api/lldb.SBTarget.html) is created:
  - If `targetCreateCommands` attribute is present, this command sequence is executed.  The currently selected target
    is assumed to have been created by these commands and will be associated with the current debugging session.
  - Otherwise, target is created from the binary pointed to by the `program` attribute.
- Target properties are configured using `args`, `env`, `cwd`, `stdio`, etc, configuration attributes.
- Breakpoints are created.
- The `preRunCommands` sequence is executed.  These commands may alter debug target configuration (e.g. args or env).
- The debuggee process is created:
  - If `processCreateCommands` attribute is present, this command sequence is executed. These are expected to have created
    a process corresponding to the debug target.
  - Otherwise, the default process creation action is performed (equivalent to the `process launch` command).
- The `postRunCommands` sequence is executed.
- Debugging until the debuggee exits, or the user requests termination.
- The `preTerminateCommands` sequence is executed.
- The debuggee is terminated (if still alive).
- If restarting the debug session, go to `preRunCommands` step.
- The `exitCommands` sequence is executed.

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

These attributes are applicable when the "attach" initiation method is selected:

|attribute          |type    |         |
|-------------------|--------|---------|
|**program**        |string  |Path of the executable file.
|**pid**            |number  |Process id to attach to.  **pid** may be omitted, in which case debugger will attempt to locate an already running instance of the program. You may also use [`${command:pickProcess}` or `${command:pickMyProcess}`](#pick-process-command) here to choose process interactively.
|**stopOnEntry**    |boolean |Whether to stop the debuggee immediately after attaching.
|**waitFor**        |boolean |Wait for the process to launch.

Operations performed for attach:
Flow during the attach sequence:
- The `initCommands` sequence is executed.
- The [debug target object](https://lldb.llvm.org/python_api/lldb.SBTarget.html) is created:
  - If `targetCreateCommands` attribute is present, this command sequence is executed.  The currently selected target
    is assumed to have been created by these commands and will be associated with the current debugging session.
  - Otherwise, target is created from the binary pointed to by the `program` attribute, if one exists.
  - Otherwise, target is created from the process specified by `pid`.
- Breakpoints are created.
- The `preRunCommands` sequence is executed.  These commands may alter debug target configuration.
- The debugger attaches to the specified process.
  - If `processCreateCommands` attribute is present, this command sequence is executed. These are expected to have
    attached debugger to the process corresponding to the debug target.
  - Otherwise, the default attach action is performed (equivalent to the `process attach` command).
- The `postRunCommands` sequence is executed.
- Debugging until the debuggee exits, or the user requests termination.
- The `preTerminateCommands` sequence is executed.
- The debuggee is detached from.
- If restarting the debug session, go to `preRunCommands` step.
- The `exitCommands` sequence is executed.

Note that attaching to a running process may be [restricted](https://en.wikipedia.org/wiki/Ptrace#Support)
on some systems.  You may need to adjust system configuration to enable it.

### Pick Process Command

The `${command:pickProcess}` or `${command:pickMyProcess}` can be used directly in the configuration for an interactive
list of processes running on the machine running Visual Studio Code:

```javascript
{
    "name": "Pick Process Attach",
    "type": "lldb",
    "request": "attach",
    "pid": "${command:pickProcess}" // Or pickMyProcess for only processes for the current user.
}
```

The `lldb.pickProcess` and `lldb.pickMyProcess` commands provide more configuration when used with input variables. The
optional `initCommands` arg let you specify lldb commands to configure a remote connection. The optional `filter` arg
lets you filter the process list to those that match the specified filter.

```javascript
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Filtered Remote Attach",
      "type": "lldb",
      "request": "attach",
      "pid": "${input:pickExampleProcess}",
      "initCommands": [ ... ], // Eg, platform select/connect commands.
    },
  ],
  "inputs": [
    {
      "id": "pickExampleProcess",
      "type": "command",
      "command": "lldb.pickProcess",
      "args": {
        "initCommands": [ ], // Eg., platform select/connect commands.
        "filter": "example" // RegExp to filter processes to.
      }
    }
  ]
}
```

## Debugging Externally Launched Code

Debugging sessions may also be started from outside of VSCode by invoking a specially formatted URI:

- **`vscode://vadimcn.vscode-lldb/launch?name=<configuration name>,[folder=<path>]`**</br>
  This will start a new debug session using the named launch configuration.  The optional `folder` parameter specifies
  the workspace folder where the launch configuration is defined.  If omitted, all folders in the current workspace will be searched.
  - `code --open-url "vscode://vadimcn.vscode-lldb/launch?name=Debug My Project"`
- **`vscode://vadimcn.vscode-lldb/launch/command?<env1>=<val1>&<env2>=<val2>&<command-line>`**</br>
  The \<command-line\> will be split into the program name and arguments array using the usual shell command-line parsing rules.
  - `code --open-url "vscode://vadimcn.vscode-lldb/launch/command?/path/filename arg1 \"arg 2\" arg3"`
  - `code --open-url "vscode://vadimcn.vscode-lldb/launch/command?RUST_LOG=error&/path/filename arg1 'arg 2' arg3"`
- **`vscode://vadimcn.vscode-lldb/launch/config?<yaml>`**</br>
  This endpoint accepts a [YAML](https://yaml.org/) snippet matching one of the above debug session initiation methods.
  The `type` and the `request` attributes may be omitted, and will default to "lldb" and "launch" respectively.
  - JSON-like YAML (if you are not quoting keys in mappings, remember to insert a space after the colon!):<br>
  `code --open-url "vscode://vadimcn.vscode-lldb/launch/config?{program: '/path/filename', args: ['arg1','arg 2','arg3']}"`<br>
  - Line-oriented YAML (`%0A` encodes the 'newline' character):<br>
   `code --open-url "vscode://vadimcn.vscode-lldb/launch/config?program: /path/filename%0Aargs:%0A- arg1%0A- arg 2%0A- arg3"`<br>


Notes:
- All URIs above are subject to normal [URI encoding rules](https://en.wikipedia.org/wiki/Percent-encoding), therefore all '%' characters must be escaped as '%25'.   A more rigorous launcher script would have done that :)<br>
- VSCode URIs may also be invoked using OS-specific tools:
  - Linux: `xdg-open <uri>`
  - MacOS: `open <uri>`
  - Windows: `start <uri>`

Examples:

### Attaching debugger to the current process (C)
```C
char command[256];
snprintf(command, sizeof(command), "code --open-url \"vscode://vadimcn.vscode-lldb/launch/config?{'request':'attach','pid':%d}\"", getpid());
system(command);
sleep(1); // Wait for debugger to attach
```

### Attaching debugger to the current process (Rust)
Ever wanted to debug a build script?
```Rust
let url = format!("vscode://vadimcn.vscode-lldb/launch/config?{{'request':'attach','pid':{}}}", std::process::id());
std::process::Command::new("code").arg("--open-url").arg(url).output().unwrap();
std::thread::sleep_ms(1000); // Wait for debugger to attach
```

(Note: You may need to update your `Cargo.toml` to build the build script with `debug`.)

```
[profile.dev.build-override]
debug = true
```

### Debugging Rust unit tests
- Create `.cargo` directory in your project folder containing these two files:
    - `config` [(see also)](https://doc.rust-lang.org/cargo/reference/config.html)
        ```TOML
        [target.<current-target-triple>]
        runner = ".cargo/codelldb.sh"
        ```
    - `codelldb.sh`
        ```sh
        #!/bin/bash
        code --open-url "vscode://vadimcn.vscode-lldb/launch/command?LD_LIBRARY_PATH=$LD_LIBRARY_PATH&$*"
        ```
- `chmod +x .cargo/codelldb.sh`
- Execute tests as normal.

### Bazel
- Create `codelldb.sh`:
    ```sh
    #!/bin/bash
    code --open-url "vscode://vadimcn.vscode-lldb/launch/command?LD_LIBRARY_PATH=$LD_LIBRARY_PATH&$*"
    ```
- `chmod +x codelldb.sh`
- `bazel run --run_under=codelldb.sh //<package>:<target>`

## RPC Server
Unfortunately, starting debug sessons via the "open-url" interface has two problems:
- It launches debug session in the last active VSCode window.
- It [does not work](https://github.com/microsoft/vscode-remote-release/issues/4260) with VSCode remoting.

For these reasons, CodeLLDB offers an alternate method of performing external launches: by adding `lldb.rpcServer` setting to a workspace
of folder configuration you can start an RPC server listening for debug configurations on a Unix or a TCP socket:
- The value is the [options](https://nodejs.org/api/net.html#net_server_listen_options_callback) object of the Node.js network server object.
- As a rudimentary security feature, you may add a "`token`" attribute to the server options above, in which case, the submitted
debug configurations must also contain `token` with a matching value.<br>
- After writing configuration data, the client must half-close its end of the connection.
- Upon completion, CodeLLDB will respond with `{ "success": true/false, "message": <optional error message> }`



### Example:
- Configuration in settings.json: `"lldb.rpcServer": { "host": "127.0.0.1", "port": 12345, "token": "secret" }`
- Launch: `echo "{ program: '/usr/bin/ls', token: 'secret' }" | netcat -N 127.0.0.1 12345`

## Remote Debugging

For general information on remote debugging please see [LLDB Remote Debugging Guide](http://lldb.llvm.org/remote.html).

### Connecting to lldb-server agent
- Run `lldb-server platform --server --listen *:<port>` on the remote machine.
- Create launch configuration similar to the one below.
- Start debugging as usual.  The executable identified by the `program` property will
be automatically copied to `lldb-server`'s current directory on the remote machine.

If you require additional configuration of the remote system, you may use `preRunCommands` sequence
to execute commands such as `platform mkdir`, `platform put-file`, `platform shell`, etc.
(see `help platform` for a list of available platform commands).

```javascript
{
    "name": "Remote launch",
    "type": "lldb",
    "request": "launch",
    "program": "${workspaceFolder}/build/debuggee", // Local path.
    "initCommands": [
        "platform select <platform>", // For example: 'remote-linux', 'remote-macosx', 'remote-android', etc.
        "platform connect connect://<remote_host>:<port>",
        "settings set target.inherit-env false", // See note below.
    ],
    "env": {
        "PATH": "...", // See note below.
    }
}
```
Note: By default, debuggee will inherit environment from the debugger.  However, this environment  will be of your
**local** machine.  In most cases these values will not be suitable on the remote system,
so you should consider disabling environment inheritance with `settings set target.inherit-env false` and
initializing them as appropriate, starting with `PATH`.

### Connecting to a gdbserver-style agent
This includes not just gdbserver itself, but also execution environments that implement the gdbserver protocol,
such as [OpenOCD](http://openocd.org/), [QEMU](https://www.qemu.org/), [rr](https://rr-project.org/), and others.

- Start remote agent. For example, run `gdbserver *:<port> <debuggee> <debuggee args>` on the remote machine.
- Create a launch configuration.
- Start debugging.
```javascript
{
    "name": "Remote attach",
    "type": "lldb",
    "request": "attach",
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

## Debugging as a Different User

While CodeLLDB does not natively support launching the debuggee as a different user, this can be easily achieved via remote debugging:

- Start `lldb-server` under the target user account, for example `sudo lldb-server platform --server --listen 127.0.0.1:12345` for root.<br>
  (A copy of lldb-server is provided in this extension's installation directory under `lldb/bin`.
  Use the "Extensions: Open Extensions Folder" command to find where extensions are located, and look for "vadimcn.vscode-lldb".)
- Add the following to your launch configuration:
```javascript
    "initCommands": [
        "platform select remote-linux", // Replace with "remote-macosx" or "remote-windows" as appropriate
        "platform connect connect://127.0.0.1:12345"
    ]
```

## Reverse Debugging

Also known as [Time travel debugging](https://en.wikipedia.org/wiki/Time_travel_debugging).  Provided you use a debugging backend that supports
[these commands](https://sourceware.org/gdb/onlinedocs/gdb/Packets.html#bc), CodeLLDB be used to control reverse execution and stepping.

As of this writing, the only known backend that works is [Mozilla's rr](https://rr-project.org/).  The minimum supported version is 5.3.0.

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
    "request": "attach",
    "targetCreateCommands": ["target create ${workspaceFolder}/build/debuggee"],
    "processCreateCommands": ["gdb-remote 127.0.0.1:<port>"],
    "reverseDebugging": true
}
```

## Inspecting a Core Dump
Use launch configuration with `target create -c <core path>` command:
```javascript
{
    "name": "Core dump",
    "type": "lldb",
    "request": "attach",
    "targetCreateCommands": ["target create -c ${workspaceFolder}/core"],
    "processCreateCommands": []
}
```

## Source Path Remapping
Source path remapping is helpful when the program's source code is located in a different directory than it was at build time (for example, if a build server was used).

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

## VSCode Commands

|                                 |                                                         |
|---------------------------------|---------------------------------------------------------|
|**Show Disassembly...**          |Choose when disassembly is shown. See [Disassembly View](#disassembly-view).
|**Toggle Disassembly**           |Choose when disassembly is shown. See [Disassembly View](#disassembly-view).
|**Display Format...**            |Choose the default variable display format. See [Formatting](#formatting).
|**Toggle Pointee Summaries**     |Choose whether to display pointee's summaries rather than the numeric value of the pointer itself. See [Pointers](#pointers).
|**Display Options...**           |Interactive configuration of the above display options.
|**Attach to Process...**         |Choose a process to attach to from the list of currently running processes.
|**Run Diagnostics**              |Run diagnostic test to make sure that the debugger is functional.
|**Generate launch configurations from Cargo.toml**|Generate all possible launch configurations (binaries, examples, unit tests) for the current Rust project.  The resulting list will be opened in a new text editor, from which you can copy/paste desired sections into `launch.json`.|
|**Command Prompt**               |Open LLDB command prompt in a terminal, for managing installed Python packages and other maintenance tasks.|
|**View Memory...**               |View raw memory starting at the specified address.|
|**Search Symbols...**            |Search for a substring among the debug target's symbols.|
|**Use Alternate Backend...**     |Choose alternate LLDB instance to be used instead of the bundled one. See [Alternate LLDB backends](#alternate-lldb-backends)


## Debugger Commands

CodeLLDB adds in-debugger commands that may be executed in the DEBUG CONSOLE panel during a debug dession:

|                 |                                                         |
|-----------------|---------------------------------------------------------|
|**debug_info**   |Provides tools for investigation of debugging information.  See `debug_info -h` for options.
|**nofail**       | `nofail <other command>` prevents errors in the execution of the specified command from aborting the current command sequence.  For example, `"postRunCommands": ["nofail process interrupt", "break set ..."]` will execute the second command even if the first one fails (e.g., because the process is already stopped).


## Debug Console

The VSCode [DEBUG CONSOLE](https://code.visualstudio.com/docs/editor/debugging#_debug-console-repl) panel serves a dual
purpose in CodeLLDB:
1. Execution of [LLDB commands](https://lldb.llvm.org/use/tutorial.html).
2. Evaluation of [expressions](#expressions).

By default, console input is interpreted as LLDB commands.  If you would like to evaluate an expression instead, prefix it with
'`?`', e.g. '`?a+2`' (Expression type preffixes are added on top of that, i.e. '`?/nat a.size()`').
Console input mode may altered via **"lldb.consoleMode": "evaluate"** setting: in this case expression evaluation will be the default,
while commands will need to be prefixed with either '`/cmd `' or '`' (backtick).

## Regex Breakpoints
Function breakpoints prefixed with '`/re `', are interpreted as regular expressions.
This causes a breakpoint to be set in every function matching the expression.
The list of created breakpoint locations may be examined using the `break list` command.

## Conditional Breakpoints
You may use any of the supported expression [syntaxes](#expressions) to create breakpoint conditions.
When a breakpoint condition evaluates to False, the breakpoint will not be stopped at.
Any other value (or expression evaluation error) will cause the debugger to stop.

## Data Breakpoints
Data breakpoints (or "watchpoints" in LLDB terms) allow monitoring memory locations for changes.  You can create data
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

## Excluded Callers

Sometimes you may want skip breakpoints when called from certain code paths.  This is particularily relevant for "on throw"
exception breakpoints in programs that use exceptions as a part of "normal" control flow.

When stopped on a breakpoint, you can right-click a frame in the CALL STACK panel and choose the "Exclude Caller" item.
Afterwards, the debugger won't stop on that breakpoint location, if the excluded caller appears anywhere in the call stack.
You can see and manage current exclusions in the EXCLUDED CALLERS panel.

## Formatting
You may change the default display format of evaluation results using the `Display Format` command.

When evaluating expressions in the DEBUG CONSOLE or in WATCH panel, you may control formatting of
individual expressions by adding one of the suffixes listed below:

|suffix |format |
|:-----:|-------|
|**c**  | Character
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
|**[\<num\>]**| Reinterpret as an array of \<num\> elements.

For example, evaluation of `var,x` will display the value of `var` formatted as hex.  It is also possible to combine
number format and array specifiers like this: `var,x[10]`.

### Pointers

When displaying pointer and reference variables, CodeLLDB will prefer to display the
value of the object pointed to.  If you would like to see the raw address value,
you may toggle this behavior using **Toggle Pointee Summaries** command.
Another way to display raw pointer address is to add the pointer variable to WATCH panel and specify
an explicit format, as described in the previous section.

## Expressions

CodeLLDB implements three expression evaluator types: "simple", "python" and "native".  These are used
wherever user-entered expression needs to be evaluated: in the WATCH panel, in the DEBUG CONSOLE (for input
prefixed with `?`) and in breakpoint conditions.<br>
By default, "simple" is assumed, however you may change this using the [expressions](#launching-a-new-process) launch
configuration property.  The default type may also be overridden on a per-expression basis using a prefix.

### Simple expressions
Prefix: `/se `<br>
Simple expressions are designed to enable performing basic arithmetic and logical operations on [formatted
views](https://lldb.llvm.org/use/varformats.html) of the debuggee variables.  For example, things like indexing an
`std::vector` or comparing `std::string` to a string literal should "just work".

The followng features are supported:
- References to variables: all identifiers are assumed to refer to variables in the debuggee current stack frame.
  The identifiers may be qualified with namespaces and template parameters (e.g. `std::numeric_limits<float>::digits`).
- Embedded [native expressions](#native-expressions): these must be delimited with `${` and `}`.
- Literals: integers, floats and strings, `True`, `False`.
- Operators: `()`, `**`, `*`, `/`, `//`, `%`, `<<`, `>>`, `~`, `&`, `^`, `|`, `==`, `!=`, `>`, `>=`, `<`, `<=`,
             `not`, `and`, `or` with the same precedence as in Python.
- Attribute access: `<expr>.<attr>`.
- Indexing: `<expr>[<expr>]`.

### Python expressions
Prefix: `/py `<br>
Python expressions support full Python syntax.  In addition to that, any identifier prefixed by `$`, will be replaced
with the value of the corresponding debuggee variable.  Such values may be mixed with regular Python variables.
For example, `/py [math.sqrt(x) for x in $arr]` will evaluate to a list of square roots of the values contained in
the array variable `arr`.

The environment in which Python expressions are executed is shared with the internal Python interpreter of the debugger
and is affected by the `script ...` command.   This may be used to import Python modules you are going to use later.
For example, in order to evaluate `math.sqrt(x)` above, you'll need to have imported the `math` package via
`script import math`.  To import Python modules on debug session startup, use `"initCommands": ["script import ..."]`.

**Technical note**<br>
Evaluation of Python expressions is performed as follows:
- First, the expression is preprocessed and all tokens starting with '$' are replaced with calls to the `__expr()` function,
  For example, the expression `[math.sqrt(x) for x in $arr]` will be re-written as `[math.sqrt(x) for x in __eval('arr')]`
- The resulting string is evaluated by the Python interpreter, with the `__eval()` function performing variable
  lookups and evaluation of native expressions, returning instances of [`Value`](#value).

### Native expressions
Prefix: `/nat `<br>
Native expressions use LLDB's built-in expression evaluators.  The specifics depend on source language of the
current debug target (e.g. C, C++ or Swift).<br>
For example, the C++ expression evaluator offers many powerful features including interactive definition
of new data types, instantiation of C++ classes, invocation of functions and class methods, and more.

Note, however, that native evaluators ignore data formatters and operate on "raw" data structures,
thus they are often not as convenient as "simple" or "python" expressions.

# Python Scripting

## Debugger API

CodeLLDB provides extended Python API via the `codelldb` module (also aliased as `debugger`),
which is auto-imported into debugger's main script context:

```python
# codelldb

def get_config(name: str, default: Any = None) -> Any:
    '''Retrieve a configuration value from the adapter settings.
        name:    Dot-separated path of the setting to retrieve.  For example, `get_config('foo.bar')`,
                 will retrieve the value of `lldb.script.foo.bar` from VSCode configuration.
        default: The default value to return if the configuration value is not found.
    '''
def evaluate(expr: str, unwrap: bool = False) -> Value | lldb.SBValue:
    '''Performs dynamic evaluation of native expressions returning instances of Value or SBValue.
        expression: The expression to evaluate.
        unwrap: Whether to unwrap the result and return it as lldb.SBValue
    '''
def wrap(obj: lldb.SBValue) -> Value:
    '''Extracts an lldb.SBValue from Value'''
def unwrap(obj: Value) -> lldb.SBValue:
    '''Wraps lldb.SBValue in a Value object'''
def create_webview(html: Optional[str] = None, title: Optional[str] = None, view_column: Optional[int] = None,
                   preserve_focus: bool = False, enable_find_widget: bool = False,
                   retain_context_when_hidden: bool = False, enable_scripts: bool = False):
    '''Create a webview panel.
        html:               HTML content to display in the webview.  May be later replaced via Webview.set_html().
        title:              Panel title.
        view_column:        Column in which to show the webview.
        preserve_focus:     Whether to preserve focus in the current editor when revealing the webview.
        enable_find_widget: Controls if the find widget is enabled in the panel.
        retain_context_when_hidden: Controls if the webview panel retains its context when it is hidden.
        enable_scripts:     Controls if scripts are enabled in the webview.
    '''
```

## Webview
A simplified interface for [webview panels](https://code.visualstudio.com/api/references/vscode-api#WebviewPanel).

```python
class Webview:
    def dispose(self):
        '''Destroy webview panel.'''
    def set_html(self, html: str):
        '''Set HTML contents of the webview.'''
    def reveal(self,  view_column: Optional[int] = None, preserve_focus: bool = False):
        '''Show the webview panel in a given column.'''
    def post_message(self, message: Any):
        '''Post a message to the webview content.'''
        interface.send_message(dict(message='webviewPostMessage', id=self.id, inner=message))
    @property
    def on_did_receive_message(self) -> Event:
        '''Fired when webview content posts a new message.'''
    @property
    def on_did_dispose(self) -> Event:
        '''Fired when the webview panel is disposed (either by the user or by calling dispose())'''
```

## Event
```python
class Event:
    def add(self, listener: Callable[[Any]]):
        '''Add an event listener.'''
    def remove(self, listener: Callable[[Any]]):
        '''Remove an event listener.'''
```

## Value
`Value` objects ([source](adapter/scripts/codelldb/value.py)) are proxy wrappers around [`lldb.SBValue`](https://lldb.llvm.org/python_api/lldb.SBValue.html),
which add implementations of standard Python operators.

## Installing Packages

CodeLLDB bundles its own copy of Python, which may be different from the version of your default Python.
As such, it likely won't be able to use third-party packages you've installed through `pip`.  In order to install packages
for use in CodeLLDB, you will need to use the **LLDB: Command Prompt** command in VSCode, followed by `pip install --user <package>`.

## Stdio in Python scripts
- `stdout` output will be sent to the Debug Console
- `stderr` output will be sent to the Output/LLDB panel

# Alternate LLDB Backends

CodeLLDB can use external LLDB backends instead of the bundled one.  For example, when debugging Swift programs,
one might want to use a custom LLDB instance that has Swift extensions built in.   In order to use an alternate backend,
you will need to provide location of the corresponding LLDB dynamic library (which must be v13.0 or later) via
**lldb.library** configuration setting.

Where to find the LLDB dynamic library:
- Linux: `<lldb root>/lib/liblldb.so.<verson>`,<br>
    `<lldb root>` is wherever you've installed LLDB, or `/usr`, if it's a standard distro package.
- MacOS: `<lldb framework>/LLDB` if built as Apple framework, `<lldb root>/lib/liblldb.<version>.dylib` otherwise.<br>
    `<lldb framework>` is typically located under `/Library/Developer/<toolchain>/.../PrivateFrameworks`.
- Windows: `<lldb root>/bin/liblldb.dll`.

Since locating liblldb is not always trivial, CodeLLDB provides the **Use Alternate Backend...** command to assist with this task.
You will be prompted to enter the file name of the main LLDB executable, which CodeLLDB will then use to find the dynamic library.

Note: Debian builds of LLDB have a bug whereby they search for `lldb-server` helper binary relative to the current
executable module (which in this case is CodeLLDB), rather than relative to liblldb (as they should).  As a result,
you may see the following error after switching to an alternate backend: "Unable to locate lldb-server-\<version\>".
To fix this, determine where `lldb-server` is installed (via `which lldb-server-<version>`), then add
this configuration entry: `"lldb.adapterEnv": {"LLDB_DEBUGSERVER_PATH": "<lldb-server path>"}`.


# Rust Language Support

CodeLLDB will attempt to locate and load LLDB data formatters provided by the Rust toolchain.  By default, the configured
toolchain of your workspace root will be used, however this can be overridden via these configuration settings:
- **lldb.script.lang.rust.toolchain** - override toolchain name, for example `beta`.
- **lldb.script.lang.rust.sysroot** - set toolchain sysroot directly, for example `/home/user/.rustup/toolchains/beta-x86_64-unknown-linux-gnu`.

To enable this feature, add `"sourceLanguages": ["rust"]` into your launch configuration.

## Cargo support

Debugging tests and benchmarks in Cargo-based Rust projects can be tricky, since the names of the output binaries generated by Cargo are not deterministic.
To address this, CodeLLDB can query Cargo for a list of its compilation outputs. To use this feature, replace the program property in your launch configuration with cargo:
```javascript
{
    "type": "lldb",
    "request": "launch",
    "cargo": {
        "args": ["test", "--no-run", "--lib"],      // Cargo command line to build the debug target
                                                    // "args": ["build", "--bin=foo"] is another option
        // Optional fields:
        "env": { "RUSTFLAGS": "-Clinker=ld.mold" }, // Extra environment variables
        "cwd": "${workspaceFolder}",                // Cargo working directory
        "problemMatcher": "$rustc",                 // Problem matcher(s) for Cargo output
        "filter": {                                 // Filter applied to compilation artifacts:
            "name": "mylib",                        // In some cases, Cargo produces multiple executable artifacts;
            "kind": "lib"                           // this helps disambiguate which one you'd like to debug.
        }
    },
    "args": ["mod::fn_name"]                        // Test name/options passed to the debug target
}
```
- You cannot use "cargo" to run your binary, only to build it. The debugger must handle the launch.
- Be as specific as possible when specifying the build target, as CodeLLDB won't know which binary to debug
  if Cargo produces more than one.
- If the program property isn't provided explicitly, CodeLLDB will set it based on the Cargo output. Alternatively,
  you can use the substitution variable ${cargo:program}.
- If no launch.json exists, CodeLLDB will use Cargo.toml in the workspace root to generate initial configurations.

# Settings

## Workspace Settings

The "Workspace Settings" term used in the document refers to the combined view of [VSCode User and Workspace settings](https://code.visualstudio.com/docs/getstarted/settings), merged according to the
[settings precedence](https://code.visualstudio.com/docs/getstarted/settings#_settings-precedence) hierarchy.

## Launch Configurations Settings
VSCode [launch configuration](https://code.visualstudio.com/docs/editor/debugging#_launch-configurations)
settings are distinct from workspace settings and are not subject to the usual settings merging described above.

However, since common defaults for all launch configurations in a project are often desired, the CodeLLDB extension
provides this feature via [`lldb.launch.*`](#default-launch-configuration-settings) setting group, which serve as defaults
for the corresponding launch configuration settings.  When a setting is specified in both locations, the values will
be merged according to on their type:
- For lists, the resulting value will be a concatenation of both sources.
- For dictionaties, the resulting value will be a combination of key-value pairs from both sources.  For equal keys,
  the launch configuration value takes precedence.
- For numbers and strings, the launch configuration value takes precedence.

### Variable Substitution in Launch Configurations
Before being sent to the debug adapter, launch configuration settings undergo expansion of
[variable references](https://code.visualstudio.com/docs/editor/variables-reference).
In addition to the standard expansions performed by VSCode, CodeLLDB also expands references to
[`${dbgconfig:<name>}`](#parameterized-launch-configurations) as well as [`${cargo:program}`](#cargo-support).

## LLDB Settings
The LLDB debugger engine also has a number of internal settings, which affect its behavior.  These
may be changed using `settings set <key> <value>` command, which may be put into any of the
`*Commands` launch configuration sequences (usually `initCommands`).

The full list of LLDB settings may be obtained by executing `settings list` command during a debug session (or in LLDB command prompt).

# Workspace Configuration Reference

## Default Launch Configuration Settings
These settings specify the default values for launch configuration setting of the same name.
|                                |                                                         |
|--------------------------------|---------------------------------------------------------|
|**lldb.launch.initCommands**    |Commands executed *before* initCommands of individual launch configurations.
|**lldb.launch.preRunCommands**  |Commands executed *before* preRunCommands of individual launch configurations.
|**lldb.launch.postRunCommands** |Commands executed *before* postRunCommands of individual launch configurations.
|**lldb.launch.exitCommands**    |Commands executed *after* exitCommands of individual launch configurations.
|**lldb.launch.env**             |Additional environment variables that will be merged with 'env' of individual launch configurations.
|**lldb.launch.envFile**         |The default envFile path.
|**lldb.launch.cwd**             |The default program working directory.
|**lldb.launch.stdio**           |The default stdio destination.
|**lldb.launch.expressions**     |The default expression evaluator.
|**lldb.launch.terminal**        |The default terminal type.
|**lldb.launch.sourceMap**       |Additional entries that will be merged with 'sourceMap's of individual launch configurations.
|**lldb.launch.breakpointMode**  |The default breakpoint resolution mode.
|**lldb.launch.relativePathBase**|The default base directory used for resolution of relative source paths.  Defaults to "${workspaceFolder}".
|**lldb.launch.sourceLanguages** |A list of source languages used in the program.  This is used to enable language-specific debugger features.

## General
|                                   |                                                         |
|-----------------------------------|---------------------------------------------------------|
|**lldb.dbgconfig**                 |See [Parameterized Launch Configurations](#parameterized-launch-configurations).
|**lldb.evaluationTimeout**         |Timeout for expression evaluation, in seconds (default=5s).
|**lldb.displayFormat**             |The default format for variable and expression values.
|**lldb.showDisassembly**           |When to show disassembly:<li>`auto` - only when source is not available.,<li>`never` - never show.,<li>`always` - always show, even if source is available.
|**lldb.dereferencePointers**       |Whether to show summaries of the pointees instead of numeric values of the pointers themselves.
|**lldb.suppressMissingSourceFiles**|Suppress VSCode's messages about missing source files (when debug info refers to files not available on the local machine).
|**lldb.consoleMode**               |Controls whether the DEBUG CONSOLE input is by default treated as debugger commands or as expressions to evaluate:<li>`commands` - treat debug console input as debugger commands.  In order to evaluate an expression, prefix it with '?' (question mark).",<li>`evaluate` - treat DEBUG CONSOLE input as expressions.  In order to execute a debugger command, prefix it with '/cmd ' or '\`' (backtick), <li>`split` - (experimental) use the DEBUG CONSOLE for evaluation of expressions, open a separate terminal for LLDB console.
|**lldb.script**                    |Configuration settings provided to Python scripts running in the context of CodeLLDB.  These may be read via [`get_config()`](#debugger-api).


## Advanced
|                       |                                                         |
|-----------------------|---------------------------------------------------------|
|**lldb.verboseLogging**|Enables verbose logging.  The log can be viewed in OUTPUT/LLDB panel.
|**lldb.rpcServer**     |See [RPC server](#rpc-server).
|**lldb.library**       |The [alternate](#alternate-lldb-backends) LLDB library to use. This can be either a file path (recommended) or a directory, in which case platform-specific heuristics will be used to locate the actual library file.
|**lldb.adapterEnv**    |Extra environment variables passed to the debug adapter.
|**lldb.cargo**         |Name of the command to invoke as Cargo.
|**lldb.terminalPromptClear**|A sequence of strings sent to the terminal in order to clear its command prompt.  Defaults to `["\n"]`.  To disable prompt clearing, set to `null`.
|**lldb.evaluateForHovers**  |Enable value preview when cursor is hovering over a variable.
|**lldb.commandCompletions** |Enable command completions in DEBUG CONSOLE.
|**lldb.reproducer**    |(deprecated) Enable capture of an LLDB reproducer.
