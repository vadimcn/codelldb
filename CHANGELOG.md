# What's New

## 0.8.7
- CodeLLDB will now attempt to auto-generate summaries for compound objects, for which there is no built-in support.
  Previously, it would fall back to displaying object's type instead.
- Fixed breakpoint resolution when dynamically-loaded modules are used.

## 0.8.6
- Filter out build scripts when looking parsing Cargo output.
- New substitution variable for launch configs: `${cargo:program}`.
- Bug fixes.

## 0.8.5
- Added support for [Cargo projects](MANUAL.md#cargo-support).
- Added support for [logpoints](https://code.visualstudio.com/updates/v1_22#_logpoints).
- Added `waitFor` property for "attach" configurations - to wait for the process to launch.
- Custom launch configuration changes (the old ways still work, but marked deprecated):
  - Use `{"request":"custom"}`, instead of `{"request":"launch", "custom":true}`.
  - Renamed `initCommands` and `preRunCommands` to
  `targetCreateCommands` and `processCreateCommands` respectively, to clarify what they do.
- `sourceLanguages` can once again be specified at the launch configuration level.  Workspace-level configuration
 is still possible via `lldb.sourceLanguages`.

## 0.8.2
- Fixed startup bug on Windows.

## 0.8.1
- Added `expressions` ∈ { `simple`, `python`, `native` } launch configuration property, which selects the default
  [expression evaluator type](MANUAL.md#expressions).
- Exception breakpoints are now language specific: "on throw" and "on catch" for C++, "on panic" for Rust.<br>
  In order to implement this, the "sourceLanguages" setting had to be moved to [workspace configuration](#workspace-configuration) level.
- Fixed watch panel bug, which, in rare circumstances, could cause display of incorrect evaluation results.

## 0.8.0
- Added [Parameterized Launch Configurations](MANUAL.md#parameterized-launch-configurations).
- Display settings such as disassembly display, default variable formats, will now be persisted across debug sessions.
- New command to toggle pointer address display setting.
- Fixed order of precedence when merging of lldb.launch... properties across multiple levels of settings.
- CodeLLDB will now pause execution upon debuggee crash (instead of terminating the debug session).

## 0.7.5
- Fixed LLDB detection on OSX and Windows.

## 0.7.4
- It is now possible to set [default values for launch configurations](MANUAL.md#workspace-configuration) per-workspace.
- The debugger will now suppress source location information if the source files does not exist on local machine (after mapping paths through `sourceMap`).
This behavior may be altered via `lldb.suppressMissingSourceFiles` configuration setting.
- Bug fixes.

## 0.7.3
- Bug fixes.

## 0.7.2
- Bug fixes.

## 0.7.1
- Bug fixes.

## 0.7.0
- The minumum VSCode version is now 1.17.
- Source maps may now contain glob wildcards.
- Source maps may now be used to suppress source location info (by setting "target" prefix to null).
- Complex variable names (e.g. statics in templated classes) may now be escaped in expressions as `${...}`.
- Breakpoints set or deleted via Debug Console commands will now be reflected in VSCode UI.

## 0.6.2
- Bug fixes.

## 0.6.1
- Bug fixes.

## 0.6.0
- The minumum VSCode version is now 1.15.
- Process state will now be updated after custom launch.
- Fixed threads display regression.
- Fixed "Add to Watch" regression.
- Experimental support for reverse-debugging via gdb-server or rr.

## 0.5.5
- Fixed Unicode handling.

## 0.5.4
- Fixed some bugs on Windows.

## 0.5.3
- Fixed fallout from VSCode 1.14 changes.
- Fixed Rust formatters after the move of String and Vec into alloc crate.

## 0.5.2
- Changed how LLDB is launched.  This should automatically fix compatibility issues with Brew Python
  and in general provide better messages when LLDB fails.
- Added 'LLDB: Run diagnostics' command for troubleshooting.

## 0.5.1
- Show pointee's summary rather than its address for pointers and references in "default" formatting
mode.  The address may still be seen when a display format override is specified, e.g. `pointer,x`.
- Bug fixes.

## 0.5.0
- The minumum VSCode version is now 1.11.
- Rust visualizers are now activated automatically (no need for `sourceLanguages: ["rust"]`).
- Added [data visualization](https://github.com/vadimcn/vscode-lldb/wiki/Data-visualization) tutorial.
- Bug fixes.

## 0.4.1
- Bug fixes.

## 0.4.0
- The minumum VSCode version is now 1.9.
- Added debugger API for HTML display.
- @keyword is no longer allowed in "simple" expressions, the '/py ...' syntax replaces that.
- Changed prefix for invoking native evaluator: `?<expr>` -> `/nat <expr>`
- Changed prefix for regex breakpoints: `/` -> `/re `.
- Fixed bugs #18, #19.

## 0.3.4
- Bug fixes.

## 0.3.3
- [Custom launch requests](MANUAL.md#custom-launch).
- Command completions in debug console.
- Windows LLDB is now supported!
- Debugger stdout is now piped to debug console.
- Fixed bug #13 (breakpoints in headers).

## 0.3.2
- Added pickProcess and pickMyProcess commands to be used with the **pid** parameter when [attaching](README.md#attaching).
- Added debug configuration snippets.
- Added Swift debugging (thanks @jesspittman!).

## 0.3.1
- Bug fixes.

## 0.3.0
- [Variable visualizers for Rust](MANUAL.md#rust-language-support).
- New [expression evaluator](MANUAL.md#expressions).
- Bug fixes.

## 0.2.2
- Bug fixes.

## 0.2.1
- Added 'terminal' launch config option. '*' in stdio config now behaves identically to null.
- Moved static variables out to their own scope.
- Disassembly in symbolless locations should work now.
- Resume debuggee after attach, unless stopOnEntry is true.

## 0.2.0
- Added [disassembly view](MANUAL.md#disassembly-view).
- Added [variable formatting](MANUAL.md#formatting).

## 0.1.3
- Added support for setting variable values (primitive types only).
- Added [regex breakpoints](MANUAL.md#regex-breakpoints).

## 0.1.2
- Infer `.exe` target extension on Windows.
- `args` may now be a string.

## 0.1.0
First released version.
