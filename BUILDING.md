# Pre-requisites:
- Node.js and npm, v8 or later.
- Python 2.7 on Linux and OSX, Python 3.6 on Windows.
- C++ compiler (GCC, clang or MSVC).
- Rust nightly.  Be sure to add an override for CodeLLDB directory so it uses the nightly.
- (Windows only) mingw-w64 toolchain (used for tests).

# Install LLDB or build it from source.

If you are building from source, clone [LLVM](https://github.com/llvm/llvm-project).  To save on build time, you may
restrict built components to just these: clang, libcxx, lldb.
```
mkdir build
cd build
cmake ../llvm -DLLVM_ENABLE_PROJECTS="clang;libcxx;lldb"
make lldb lldb-server
```
Please note that on Windows, LLDB is expected to have been build with MSVC compiler.

CodeLLDB build scripts expect to find the following files under ${LLDB_ROOT}:
```
bin/
  lldb[.exe]
  lldb-server[.exe]
  liblldb.dll (Winodws)
lib/
  liblldb.so.<version> (Linux)
  liblldb<version>.dylib (OSX)
  python2.7/     (Linux and OSX)
  site-packages/ (Windows)
    <python files>
```

# Configure CodeLLDB:
```
npm install
mkdir build  # (directory can be changed, but tasks.json assumes it's "build")
cd build
cmake .. -DLLDB_ROOT=<path to LLDB directory>
```
- `LLDB_ROOT` specifies the directory where required LLDB components will be pulled from.
- `LLDB_EXECUTABLE` specifies name of the system-provided LLDB, if it isn't just "`lldb`" (for example, on Ubuntu it's usually `lldb-<version>`).

3. Useful targets:
- `extension` - build VSCode extension.
- `tests` - build extension tests.
- `debuggee` - build the debuggee project (used for debbugging and tests).
- `adapter` - build classic adapter.
- `codelldb` - build native adapter.
- `cargo_test` - run `cargo test` on all codelldb crates.
- `dev_debugging` - build extension, adapters, debuggee and other stuff needed for debugging extension directly out of the build directory.
After building this target you can run `code --extensionDevelopmentPath=${workspaceFolder}/build` to try out the extension.
- `check_` { `classic` | `bundled` | `native` } - build and test the specified adapter type.
- `check` - build adapter and run tests for all adapter types.
- `vsix_portable` - build VSIX package containing only the "classic" adapter.
- `vsix_full` - build VSIX package including all required native binaries (for the current platform).
- `xclean` - extra-thorough cleaning.  Useful in cases of build problems caused by stale dependencies.
