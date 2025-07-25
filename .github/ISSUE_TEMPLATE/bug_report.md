---
name: Bug report
about: Before filing a new report, please review the Troubleshooting page in CodeLLDB Wiki.
---
<!-- ⚠️ Before filing a new report, please review https://github.com/vadimcn/codelldb/wiki/Troubleshooting ⚠️ -->

OS: <!-- including version -->
VSCode version:  <!-- from Help/About -->
CodeLLDB version: <!-- from the Extensions panel -->
Compiler: <!-- Name (rustc/gcc/clang) and version of the compiler you are using -->
Debuggee: <!-- What kind of a binary you are debugging? ❶ -->

<!-- What is the problem and how did you get there -->

<details> <!-- If reporting a debugger crash or an internal error, please consider providing a verbose log ❷ -->
<summary>Verbose log</summary>

```
Log goes here
```

</details>


<!--
❶ A target triple (<architecture>-<os>-<abi>, e.g. "aarch64-linux-gnu" or "x86_64-windows-msvc")  would be the best, otherwise,  provide as much detail as you know.  If on Windows, please find out whether the debuggee uses DWARF debug info (gnu ABI) or PDB (msvc ABI) - this is important!

❷ How to capture a verbose log:
  1. Add "lldb.verboseLogging":true to your workspace configuration,
  2. Reproduce the problem,
  3. Copy debug output from the Output/LLDB panel.
-->
