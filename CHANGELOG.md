# Release Notes

# 1.11.5


### New
- Added the `nofail` command, which can be used in command sequences to suppress errors and prevent the sequence from being aborted.<br>
  For example, `"postRunCommands": ["nofail process interrupt", "break set ..."]` will execute the second command even if the first one fails
  (e.g., because the process is already stopped).

### Fixed
- #1262: Exits with an error if file specified by envFile in launch configuration doesn't exist.
- #1267: Could not initialize Python interpreter - some features will be unavailable.

### Changed
- Updated bundled LLDB to v20.1.0
- The minimal required MacOS version for x86_64 is now 10.12

# 1.11.4

### Fixed
- #1044: VS Code doesn't clear the extension folder after upgrading or uninstalling
- #1228: SourceMaps stopped working for bazel-built processes on version 1.11.2 and 1.11.3

### Changed
- Updated bundled LLDB to v19.1.0

# 1.11.3

### Fixed
- #1217: Crash on remote debugging qemu after upgrading to 1.11.2
- #1221: Unable to resolve liblldb symbols
- #1225: Fix i8/u8 variable formatting

# 1.11.2

### New
- @puremourning has [enhanced data breakpoints](https://github.com/vadimcn/codelldb/pull/1161).
  It is now possible to set data breakpoints of arbitrary sizes, as well as use them in more contexts than before.

### Changed
- Deprecating "custom" launch configurations, as the same functionality may be achieved with **"request": "launch"** + **"targetCreateCommands"** and **"processCreateCommands"**<br>
 **"request": "custom"** is still accepted, however it will behave the same as "launch".
- Improved "no-debug" mode startup time by disabling symbol preloading.

### Fixed
- #1064: Target creation should not be required before gdb-remote
- #1177: Debugger panicks if Python cannot be initialized
- #1205: Missing string escaping
- #1209: display_html throws an exception
- #1212: Can't debug ... with an External terminal

# 1.11.1

### New
- Updated embedded Python to v3.12.
- Added support for [Step Into Targets](https://code.visualstudio.com/updates/v1_46#_step-into-targets).
  When debugging statements such as `foo(bar(), baz())`, this allows stepping directly into `foo`, bypassing `bar` and `baz`.
- Added support for the `restart` request: This enables restarting the debuggee without ending the current session,
  making restarts faster by reusing the same debug adapter instance and cached debug info from the debuggee binary.<br>
  Note that, because the session is retained, the **exitCommands** sequence will not run before terminating the old instance
  of the debuggee. To address this, a new **preTerminateCommands** sequence has been added.
  Additionally, **initCommands** will not be re-executed, while **preRunCommands** and **postRunCommands** will be.
- Added "cwd" attribute to Cargo configuration.
- Add `--color=always` when running Cargo.

### Fixed
- #1113: Disassembly does not show until call stack is clicked
- #1126: Highlight the current hit breakpoint
- Restored compatibility with liblldb v17

# 1.11.0

### New
- Updated bundled LLDB to v19.1.0.
- The Python module implementing the CodeLLDB Python API is now called `codelldb` (aliased to `debugger` for backward
  compatibility).
- Python scripts running in the context of CodeLLDB can now read workspace configuration settings stored under
  the `lldb.script` namespace via `codelldb.get_config()`.

### Changed
- To reduce the maintenance burden, support for the Rust language service and custom data formatters in CodeLLDB has
  been removed. The constant breaking changes in LLDB's language service API, along with Rust's evolving internal
  representation of `std::` types, have made it increasingly difficult to maintain these updates. Future versions of
  CodeLLDB will be based on stock LLDB, without the Rust language service. Rust data types will still have partial
  support via the data formatters provided by `rustc`, but custom formatters will no longer be maintained.

# 1.10.0

## New
- Updated bundled LLDB to v17.0.0

## Fixed
- #954: VSCode call stack doesn't work when instruction pointer is invalid
- #958: Excluded Callers feature not working in Dev Container
- #853: Display Whole String in Debug Console
- #980: Global variables are miscategorized as static

# 1.9.2

## New
- Implemented [Excluded Callers](MANUAL.md#excluded-callers) feature, similar to the
  [one in Javascript debugger](https://code.visualstudio.com/updates/v1_64#_javascript-debugging).
- Added [create_webview()](MANUAL.md#webview) Python API, which allows scripts to create and manipulate VSCode Webviews.
  This function supersedes functionality of the older `display_html` API.
- Enabled conditions on exception breakpoints.

# 1.9.1

## New
- Implemented support for [envFile](https://github.com/vadimcn/codelldb/issues/866).
- Added `breakpointMode` setting: when this is set to `file`, breakpoints will be resolved using file name only, which
  is similar to how `breakpoint set -f <filename> -l <line>` command works in CLI LLDB.  This relieves the need
  of setting up `sourceMap`; however, this is at the expense of potentially hitting unexpectd breakpoints
  if there is more than one source file of the same name in the project.
- `targetCreateCommands` and `processCreateCommands` are now allowed in for `launch` and `attach` requests.  When
  specified, these command sequences over-ride the default logic for target and process creation.

## Fixed
- #761: Error: there is no registered task type 'codelldb.cargo'
- #776: Error: there is no registered task type 'codelldb.cargo'
- #891: Incorrect matcher
- #904: Cannot see VecDeque values in "Variables" panel after insert
- #911: Vec in sidebar shows wrong (old) value- #920: Rust: local variables not updated during debugging
- #915: Pick(My)Process not working

# 1.9.0

## New
- Updated bundled LLDB to v16.0.0
- It is now possible to combine number format specifiers (`foo,x`) and "reinterpret as array" speficiers (`foo,[10]`)
  together: `foo,x[10]` (Feature request #851).
- Added support for native VSCode [disassmbly view](https://devblogs.microsoft.com/cppblog/visual-studio-code-c-july-2021-update-disassembly-view-macro-expansion-and-windows-arm64-debugging/#disassembly-view) (thanks @puremourning!).

## Fixed
- #813: Mixed GAS/Intel syntax in disassembly view.
- #842: Syntax error in conditional breakpoint.
- #840: Make the whole command string readable in the "Select a process" dropdown window.

# 1.8.1

## Fixed
- #777: Json parsing error when debugging rust

# 1.8.0

## New
- Updated LLDB to 15.0.0
- Added experimental `split` option to the [`consoleMode`](MANUAL.md#general) config setting.  In this mode the debug console
  will be used for evaluation of expressions and a separate terminal will be created for input of LLDB commands.

## Changed
- ["Simple" expressions](MANUAL.md#simple-expressions)" now use a proper parser, which should make syntax error messages less confusing.
- `${`...`}`-delimited expressions embedded in Simple and Python expressions now may conatin full Native expressions,
not just variable names, which the case previously.
- The `show_debug_info` [command](MANUAL.md#debugger-commands) has been renamed to `debug_info`, with sub-commands `list` and `show`.

# 1.7.4

## Fixed
- #745: Bug: didn't catch panics in Rust code

# 1.7.3

## Fixed
- #734: Cargo task hangs.
- #738: Cargo artifacts are not filtered.

# 1.7.2

## Fixed
- #731: Crash on debug session startup.

# 1.7.1

## Changed
- Renamed `debug_info` command to `show_debug_info`.

## New
- Cargo is now executed as a task, which allows applying problem matchers to its output.
- Added `lldb.cargo` configuration setting to allow overriding command invoked as Cargo.
- Added `cargo.env` attribute in launch configs to allow passing custom environment variables to Cargo.
- Added "View Memory" command, which allows viewing raw memory at an arbitrary address.
- Added "Search Symbols" command, which allows searching target's symbols.
- Initialize LLDB's `target.process.thread.step-avoid-regexp` setting for Rust programs, to avoid stepping into Rust std library.

## Fixed
- #647: Cannot display Python string containing non-ASCII characters.
- #718: Large containers take too long to display.
- #726: Cannot display Rust String's and Vec's (due to data layout change in std).

# 1.7.0

## Changed
- CodeLLDB configuration settings are now split into three groups: basic, advanced, and launch configuration defaults.

## New
- Updated LLDB to 14.0.0
- Added support for VSCode [memory viewer](https://code.visualstudio.com/updates/v1_64#_viewing-and-editing-binary-data).
- Added UI for [console mode](MANUAL.md#debug-console) switching.

## Fixed
- #584: Rust - With Simple Function, LLDB Debugger variables duplicated and random?
- #633: Rust now emits with a new PDB language identifier, breaking debugging on Windows

# 1.6.10

## Fixed
- #568: Viewing a struct with union:s in C++ doesn't show all the member variables
- #573: Installing package flags "Unauthorized"

# 1.6.9

## Fixed
- #465: Incompatibility with arm64 macOS Monterey.

# 1.6.8

## Changed
- Okay, LLDB 12.0.1 seems too broken.  Restored it to v13.0.0.

## Fixed
- #527: Breakpoints no longer triggering in library when using version 1.6.7
- #540: Rust printing of enums not working [for -windows-msvc targets]
- #337: Visualizing Vec<(f32,f32)> is showing as pointer

# 1.6.7

## Changed
- Rolled the bundled LLDB back to v12, because crash reports started coming from users after upgrade to v13.

## New
- Added support for inline breakpoints.
- Added support for LLDB [reproducers](https://lldb.llvm.org/design/reproducers.html).

## Fixed
- Bug #512: External terminal no longer works since updating to v1.6.6
- Bug #519: Attaching to remote process using debugserver on Mac/iOS is not working.
- Bug #522: Conditional breakpoints don't trigger when empty logMessage is supplied.

# 1.6.6

## New
- Updated LLDB to 13.0.0
- Added [`debug_info`](MANUAL.md#debugger-commands) LLDB command, that makes it easier to determine which modules have debug info available.

## Fixed
- Bug #474: image dump sections causes the plugin to hang
- Bug #480: Panic: 'assertion failed: addr.is_valid()'

# 1.6.5

## Fixed
- Bug #327: Enums are not correctly displayed on MacOS.
- Bug #451: Debug Console doesn't work with version 1.6.4 on Windows.
- Bug #454: readMemory request does not accept negative offset.

## New
- Rust windows-msvc binaries built using nightly-2021-06-05 or later compiler will have their enums displayed correctly 🎉.  (This will be in stable rustc 1.54)
- Upon completing a request, the [RPC server](MANUAL.md#rpc-server) will now respond with a status message.

# 1.6.4

## Fixed
- Bug #411: Cannot launch and connect to debugserver on Mac
- Bug #412: std::collectionHashMap/Set are not displayed (on x86_64-pc-windows-msvc)
- Bug #435: debug session exited unexpectedly
- Bug #438: Supply memoryReference for variables
- Bug #439: Debug session fails if I "watch" for a certain expression
- Bug #440: Nicer watch window messages with "native" expressions
- Bug #442: Rust std::collection::HashMap has no pretty printing

# 1.6.3

## Fixed
- Bug #424: 1.6.2 failure stopping at breakpoints
- Bug #428: Debugger fails to start (Fresh install / macOS 10.13)

# 1.6.2

## Fixed
- Bug #417: Loaded modules not shown.
- Fixed visualizers for Rust 1.48+ hashmaps.

## New
- Added support for Apple Silicon.
- The bundled LLDB is now compiled with support for X86, ARM, AVR, RISCV, MSP430 and WebAssembly architectures.
- Added [RPC server](MANUAL.md#rpc-server) for "external" launching.
- Implemented new data watchpoint options (read vs write vs read/write).

# 1.6.1

## Fixed
- Bug #395 - Size and content of std::vector is not adjusted correctly after push_back.
- Bug #394 - Debug adapter crash when hit breakpoint.

## New
- Added "LLDB Command Prompt" command, which opens LLDB command prompt in a terminal.  This is mainly intended for
  managing installed Python packages (via the `pip` command).
- Added `"lldb.evaluateForHovers"` configuration setting, which allows to disable expression evaluation for mouse hovers.
  This is intended to mitigate problems similar to the one described in #353, triggered by auto-evaluation of expressions.
- Added `"lldb.commandCompletions"` configuration setting, which allows to disable command completions in the debug console.
  Similarly to the previous one, this is for mitigation of LLDB crashes triggered by completions.

# 1.6.0

## Changed
- I've decided to stop trying to use external Python installations with CodeLLDB.  Bugs keep coming in, and it seems
  that the diversity of Python variants out there is just too big.<br>
  As of this version, a minimal Python installation will be bundled with CodeLLDB (courtesy of PyOxidizer project!), so
  users won't have to worry about installing it separately.
- The bundled LLDB is now based on version 11.0.   This fixes a number of problems in parsing C++ debug info, including
  returning wrong template parameters for some types and crashes during expression evaluation.
- Due to problems it causes for some shells, the terminal prompt clearing feature will now be disabled by default.
  Those who wish to keep using it, can re-enable it by adding `"lldb.terminalPromptClear": ["\n"]` to their
  user/workspace configuration files.
- Added Objective C++ and Zig to the list of supported languages.

## Fixed
- Source file paths on Windows will now follow casing of the file system.  This should fix a number of issues where
  VSCode would not display the current execution location in the editor.
- Updated Rust HashMap/HashSet formatter to account for the recent memory layout change.

# 1.5.3

## Fixed
- Bugs #312, #318: In v1.5.2 a new method of clearing the terminal prompt had been introduced; unfortunately, it seems
to have caused problems for some shells (fish, zsh).  This should be resolved now.  If you still experience problems,
you can use the newly added `lldb.terminalPromptClear` setting to override string sequence used to clear the prompt,
or to disable prompt clearing altogether.

## New
- Added `lldb.terminalPromptClear` setting.

# 1.5.2

## Fixed
- Bug #276: Running lldb python scripts only outputs to OUTPUT(lldb) tab.
- Bug #286: "Run Without Debugging" doesn't wait for process to finish.
- Bug #297: Breakpoints disappear when debug symbols have relative path outside ${workspaceFolder}, or paths that need mapping.
- Fixed compatibility with Eclipse Theia IDE (thanks @dschafhauser!)

## Changed
- Updated bundled LLDB to v10.0.1

## New
- Added '/cmd ' prefix (in addition to backtick) for executing lldb commands when debug console is set to `evaluate` mode.

# 1.5.1

## Fixed
- Bug 270: CodeLLDB requires too new liblzma on OSX

# 1.5.0

## Fixed
- Bug #252: Cannot Attach: Could not send event to DebugSession: "Full(..)"
- Bug #253: Rust conditional breakpoints: `usize` is a string?
- Debugging inside docker containers should work now.

## Changed
- Removed "classic" adapter.
- Improved platform package validation after download.

# 1.4.5

## Fixed
- Improved compatibility with Anaconda Python on Windows.
- Fixed parsing of Python versions involving beta releases.

## Other
- [Version 5.3](https://github.com/mozilla/rr/releases/tag/5.3.0) of Mozilla's [rr](https://rr-project.org/) has been released a few days ago.  It seems to work pretty well with CodeLLDB's [reverse debugging](MANUAL.md#reverse-debugging) support.
- This is likely to be the last version supporting "classic" adapter.

# 1.4.4

## Fixed
- Bug #238: Unable use attach snippet

# 1.4.3

## Fixed
- Bug #231: v1.4.1 freezes program

# 1.4.2

## Fixed
- Bug #229: Cargo invocation has failed: Error: spawn ENOMEM

# 1.4.1

## Fixed
- Bug #221: No-debug launch mode doesn't work.
- Fixed "reinterpret as array" format specifier (var,[length]).

## New
- Rust visualizers now support `HashMap` and `HashSet`.
- The `.../command` [URL handler](MANUAL.md#starting-debug-session-outside-of-vscode) now supports setting debuggee environment variables.
- Added support for armv7 platform (Raspberry Pi, etc).

# 1.4.0

## Changed
- In preparation for [Python 2 fading into the sunset](https://pythonclock.org/), all supported platforms now require Python 3.3 or later.

## New
- Added support for [data breakpoints](https://code.visualstudio.com/updates/v1_38#_breaking-when-value-changes-data-breakpoints).
- Added "Attach to Process..." command for quick attaching without having to create a debug configuration.
- Added URL handler for [starting a debug session from outside of VSCode](MANUAL.md#starting-debug-session-outside-of-vscode).<br>
  Rust users: please take note - I believe this may provide a more convenient way of debugging the unit tests.

# 1.3.0

## Fixed
- Redirection to the integrated terminal now works on Windows too.

## Changed
- [Native adapter](#heads-up-codelldb-is-moving-to-native-code) is now the default.  You can still use 'classic' or 'bundle' by setting the `lldb.adapterType` configuration option.
- "integrated" is now the default value for the "terminal" launch config property.

## New
- Loaded modules viewlet: rather than printing loaded modules notifications in the Debug Console view, modules are now displayed in a separate tab in the Debug view.
- `lldb.consoleMode` setting, which controls whether the debug console input is by default treated as debugger commands or as expressions to evaluate.
- Added support for [Jump to to cursor](https://code.visualstudio.com/updates/v1_36#_jump-to-cursor) command (thanks @ntoskrnl7!).

# 1.2.3

## New
- New UI for display settings (status bar and "Display Options..." command).
- Added support for configurable external LLDB backends (native adapter only).

## Changed
- Updated bundled LLDB to v8.0 final.

## Fixed
- Bug #173 - Debugger module is not auto-imported when native adapter is used.
- Native adapter panics in rare cases when formatting Python tracebacks.

# 1.2.2

### Fixed
- Debug configuration generation from Cargo.toml when using recent Cargo versions.

### Fixed (native adapter only)
- LLDB command completions inserting duplicate tokens in some cases.
- Remote debugging when using QEMU debug stub.
- Spurious stop events at the beginning of a debug session.

### New (native adapter only)
- Implemented hit conditions on breakpoints.
- More informative error messages when displaying optimized-out variables, invalid pointers, etc.
- Announce executed scripts (e.g. initCommands, preRunCommands), for easier attribution of script errors.
- Support ",[\<number\>]" format specifier, which reinterprets the displayed value as an array of \<number\> elements.

### Heads up: CodeLLDB is moving to native code.

Up until now, CodeLLDB's debug adapter has been based on whatever version of the LLDB was installed on the local machine,
with Python scripts providing the glue between LLDB API and VS Code. This arrangement has its benefits:
the extension can be very compact and platform-independent. The flip side of using an externally-provided LLDB, is that it may
happen to be quite old and buggy.  There had been quite a few problems reported because of that.  I've also been somewhat
dissatisfied with CodeLLDB's performance and stability, which I attribute to the use of Python in a project that has
long grown past being "just a simple script".<br>

As a consequence, I've decided to try a new approach:
- Pre-built LLDB binaries will be provided with the extension. This will ensure that it is used with the same
version of LLDB engine as it was tested with. (In order to reduce the download size, native binaries will not be included
in the initial installation package published on VS Code Marketplace.  Instead, a smaller, platform-targeted package will
be downloaded on first use.)
- Most of Python code had been ported to a statically-typed compiled language (Rust).

For now, both implementations of the debug adapter will exist in parallel.
You can choose which one is used by setting `lldb.adapterType` to either `classic` or `native` in your workspace settings.
In a few versions, I plan to make `native` the default, and then, eventually, the only option.

Please give the `native` adapter a try and let me know how that worked for you, and, especially, if it didn't.  Thanks!

# 1.2.1

### Changed
- The minimum supported VSCode version is now 1.30.

### Fixed
- Source maps.
- Python detection on Windows.
- Debug adapter process is sometimes left running after the end of a debug session.
- Adjusted Rust visualizer for libstd changes in v1.33.

# 1.2.0

### New
- [Beta] Introduced "bundled" and "native" debug adapter types (in addition to "classic"): if `lldb.adapterType` confguration
setting is set to either of those values, CodeLLDB will download and use a custom build of LLDB and use it instead of the
system-provided one.

### Fixed
- Misc bug fixes.

# 1.1.0
- The minimum supported VSCode version is now 1.23.
- Due to deprecation of VSCode's `previewHtml` command, the signature of `display_html` API had to change: HTML markup
must now be provided in the first parameter; lazy content generation via `register_content_provider` is no longer supported.
- Bug fixes.

# 1.0.0
- The pace of changes has been slowing down as of late, I think this is about time to declare a v1.0!
- Added Rust visualizers for Box, Rc, Arc, Mutex, Cell, RefCell.
- Bug fixes.

# 0.8.9
- Bug fixes.

# 0.8.8
- Added [`postRunCommands`](MANUAL.md#launching).
- Bug fixes.

# 0.8.7
- CodeLLDB will now attempt to auto-generate summaries for compound objects, for which there is no built-in support.
  Previously, it would fall back to displaying object's type instead.
- Fixed breakpoint resolution when dynamically-loaded modules are used.

# 0.8.6
- Filter out build scripts when looking parsing Cargo output.
- New substitution variable for launch configs: `${cargo:program}`.
- Bug fixes.

# 0.8.5
- Added support for [Cargo projects](MANUAL.md#cargo-support).
- Added support for [logpoints](https://code.visualstudio.com/updates/v1_22#_logpoints).
- Added `waitFor` property for "attach" configurations - to wait for the process to launch.
- Custom launch configuration changes (the old ways still work, but marked deprecated):
  - Use `{"request":"custom"}`, instead of `{"request":"launch", "custom":true}`.
  - Renamed `initCommands` and `preRunCommands` to
  `targetCreateCommands` and `processCreateCommands` respectively, to clarify what they do.
- `sourceLanguages` can once again be specified at the launch configuration level.  Workspace-level configuration
 is still possible via `lldb.sourceLanguages`.

# 0.8.2
- Fixed startup bug on Windows.

# 0.8.1
- Added `expressions` ∈ { `simple`, `python`, `native` } launch configuration property, which selects the default
  [expression evaluator type](MANUAL.md#expressions).
- Exception breakpoints are now language specific: "on throw" and "on catch" for C++, "on panic" for Rust.<br>
  In order to implement this, the "sourceLanguages" setting had to be moved to [workspace configuration](#workspace-configuration) level.
- Fixed watch panel bug, which, in rare circumstances, could cause display of incorrect evaluation results.

# 0.8.0
- Added [Parameterized Launch Configurations](MANUAL.md#parameterized-launch-configurations).
- Display settings such as disassembly display, default variable formats, will now be persisted across debug sessions.
- New command to toggle pointer address display setting.
- Fixed order of precedence when merging of lldb.launch... properties across multiple levels of settings.
- CodeLLDB will now pause execution upon debuggee crash (instead of terminating the debug session).

# 0.7.5
- Fixed LLDB detection on OSX and Windows.

# 0.7.4
- It is now possible to set [default values for launch configurations](MANUAL.md#workspace-configuration) per-workspace.
- The debugger will now suppress source location information if the source files does not exist on local machine (after mapping paths through `sourceMap`).
This behavior may be altered via `lldb.suppressMissingSourceFiles` configuration setting.
- Bug fixes.

# 0.7.3
- Bug fixes.

# 0.7.2
- Bug fixes.

# 0.7.1
- Bug fixes.

# 0.7.0
- The minimum supported VSCode version is now 1.17.
- Source maps may now contain glob wildcards.
- Source maps may now be used to suppress source location info (by setting "target" prefix to null).
- Complex variable names (e.g. statics in templated classes) may now be escaped in expressions as `${...}`.
- Breakpoints set or deleted via Debug Console commands will now be reflected in VSCode UI.

# 0.6.2
- Bug fixes.

# 0.6.1
- Bug fixes.

# 0.6.0
- The minimum supported VSCode version is now 1.15.
- Process state will now be updated after custom launch.
- Fixed threads display regression.
- Fixed "Add to Watch" regression.
- Experimental support for reverse-debugging via gdb-server or rr.

# 0.5.5
- Fixed Unicode handling.

# 0.5.4
- Fixed some bugs on Windows.

# 0.5.3
- Fixed fallout from VSCode 1.14 changes.
- Fixed Rust formatters after the move of String and Vec into alloc crate.

# 0.5.2
- Changed how LLDB is launched.  This should automatically fix compatibility issues with Brew Python
  and in general provide better messages when LLDB fails.
- Added 'LLDB: Run diagnostics' command for troubleshooting.

# 0.5.1
- Show pointee's summary rather than its address for pointers and references in "default" formatting
mode.  The address may still be seen when a display format override is specified, e.g. `pointer,x`.
- Bug fixes.

# 0.5.0
- The minimum supported VSCode version is now 1.11.
- Rust visualizers are now activated automatically (no need for `sourceLanguages: ["rust"]`).
- Added [data visualization](https://github.com/vadimcn/codelldb/wiki/Data-visualization) tutorial.
- Bug fixes.

# 0.4.1
- Bug fixes.

# 0.4.0
- The minimum supported VSCode version is now 1.9.
- Added debugger API for HTML display.
- @keyword is no longer allowed in "simple" expressions, the '/py ...' syntax replaces that.
- Changed prefix for invoking native evaluator: `?<expr>` -> `/nat <expr>`
- Changed prefix for regex breakpoints: `/` -> `/re `.
- Fixed bugs #18, #19.

# 0.3.4
- Bug fixes.

# 0.3.3
- [Custom launch requests](MANUAL.md#custom-launch).
- Command completions in debug console.
- Windows LLDB is now supported!
- Debugger stdout is now piped to debug console.
- Fixed bug #13 (breakpoints in headers).

# 0.3.2
- Added pickProcess and pickMyProcess commands to be used with the **pid** parameter when [attaching](README.md#attaching).
- Added debug configuration snippets.
- Added Swift debugging (thanks @jesspittman!).

# 0.3.1
- Bug fixes.

# 0.3.0
- [Variable visualizers for Rust](MANUAL.md#rust-language-support).
- New [expression evaluator](MANUAL.md#expressions).
- Bug fixes.

# 0.2.2
- Bug fixes.

# 0.2.1
- Added 'terminal' launch config option. '*' in stdio config now behaves identically to null.
- Moved static variables out to their own scope.
- Disassembly in symbolless locations should work now.
- Resume debuggee after attach, unless stopOnEntry is true.

# 0.2.0
- Added [disassembly view](MANUAL.md#disassembly-view).
- Added [variable formatting](MANUAL.md#formatting).

# 0.1.3
- Added support for setting variable values (primitive types only).
- Added [regex breakpoints](MANUAL.md#regex-breakpoints).

# 0.1.2
- Infer `.exe` target extension on Windows.
- `args` may now be a string.

# 0.1.0
First released version.
