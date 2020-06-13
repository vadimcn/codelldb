# Prerequisites:
- Node.js and npm, v8 or later.
- Python 3.5 or later.
- C++ compiler (GCC, clang or MSVC).
- Rust nightly 2020-05-04 or later.  Be sure to add an override for CodeLLDB directory so it uses the nightly.
- (Windows only) mingw-w64 toolchain (used for tests).

# Install LLDB or build it from source.

If you are building from source, clone [LLVM](https://github.com/llvm/llvm-project).  To save on build time, you may
want to restrict the built components to just these: clang, libcxx, lldb.
```
mkdir build
cd build
cmake ../llvm -DLLVM_ENABLE_PROJECTS="clang;libcxx;lldb"
make lldb lldb-server
```
Please note that on Windows, LLDB is expected to have been built with the MSVC compiler.

Package liblldb and its dependencies into a zip archive.
(This gets rather tricky if you want to obtain a self-contained package, however for local testing just zipping
bin/ and lib/ directories will do.)

# Configure CodeLLDB:
```
npm install
mkdir build  # (directory can be changed, but tasks.json assumes it's "build")
cd build
cmake .. -LLDB_PACKAGE=<path>
```
- `LLDB_PACKAGE` specifies zip archive containing LLDB files.

3. Useful targets:
- `extension` - build VSCode extension.
- `tests` - build extension tests.
- `debuggee` - build the debuggee project (used for debbugging and tests).
- `codelldb` - build the debug adapter.
- `cargo_test` - run `cargo test` on all codelldb crates.
- `dev_debugging` - build extension, adapter, debuggee and other stuff needed for debugging extension directly out of the build directory.
After building this target you can run `code --extensionDevelopmentPath=${workspaceFolder}/build` to try out the extension.
- `check` - build adapter and run all tests.
- `vsix_bootstrap` - build VSIX package containing only the VSCode extension.
- `vsix_full` - build VSIX package including all required native binaries (for the current platform).
- `xclean` - extra-thorough cleaning.  Useful in cases of build problems caused by stale dependencies.
