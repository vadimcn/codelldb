# Release Notes

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
- Added [data visualization](https://github.com/vadimcn/vscode-lldb/wiki/Data-visualization) tutorial.
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
