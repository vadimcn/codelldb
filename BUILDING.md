# Prerequisites:
- Node.js and npm, v8 or later.
- Python 3.5 or later.
- C++ compiler (GCC, clang or MSVC).
- Rust nightly 2019-10-15 or later.  Be sure to add an override for CodeLLDB directory so it uses the nightly.
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
  liblldb.dll            (Windows)
lib/
  liblldb.so.<version>   (Linux)
  liblldb<version>.dylib (OSX)
  python3/               (Linux and OSX)
  site-packages/         (Windows)
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
- `codelldb` - build the debug adapter.
- `cargo_test` - run `cargo test` on all codelldb crates.
- `dev_debugging` - build extension, adapters, debuggee and other stuff needed for debugging extension directly out of the build directory.
After building this target you can run `code --extensionDevelopmentPath=${workspaceFolder}/build` to try out the extension.
- `check` - build adapter and run all tests.
- `vsix_portable` - build VSIX package containing only the "classic" adapter.
- `vsix_full` - build VSIX package including all required native binaries (for the current platform).
- `xclean` - extra-thorough cleaning.  Useful in cases of build problems caused by stale dependencies.
