# Prerequisites
- Node.js and npm, v10 or later.
- Python 3.5 or later.
- C++ compiler native to your platform (GCC, clang or MSVC).
- Rust 1.61 or later.
- (Windows only) mingw-w64 toolchain (used for tests).

# Clone the repo
```bash
git clone ${from} codelldb
cd codelldb
```

# LLDB package

This is a zip archive containing the subset of LLDB files which CodeLLDB needs at runtime.
Pre-built packages are published here: https://github.com/vadimcn/llvm-project/releases.

# Configure

- On Windows, you should do this from "x64 Native Tools Command Prompt", so that MSVC environment is set up correctly.
- On Mac, I recommend starting a new shell via `xcrun --sdk macosx zsh`

```bash
cd codelldb
mkdir build  # The build directory may be changed, however tasks.json assumes it's "build" under the project root
cd build
cmake .. -DCMAKE_TOOLCHAIN_FILE=../cmake/toolchain-x86_64-linux-gnu.cmake -DLLDB_PACKAGE=${path to LLDB package}
```
On other platforms, alter the toolchain file path accordingly.  You *will* get linker errors if you don't use the toolchain file.

If building LLDB from source, you can also do this:
```bash
cmake .. -DCMAKE_TOOLCHAIN_FILE=../cmake/toolchain-x86_64-linux-gnu.cmake -DLLDB_PACKAGE=${path to LLVM build directory} -DLLVM_SOURCE_DIR=${path to LLVM source}
```
The last parameter may be omitted if the LLVM build directory is located in the source tree.

## VSCode
If you intend to run and debug tests in VSCode, you may want to create a symlink from `<souce>/.cargo/config.toml`
to `<build>/.cargo/config.toml`.

# Build
```bash
cd build
make ${target}
```

# Test
```bash
make check
```
You can also execute specific tests via
```bash
ctest -N             # List available tests
ctest -V -R ${regex} # Run test(s) matching the regex
```
- `adapter:<target>` - tests core debug adapter functionality using a debuggee built for the specified target
  (e.g., on Windows, either x86_64-pc-windows-gnu or x86_64-pc-windows-msvc).
- `cargo_test` - execute cargo tests in all crates.
- `dependencies:<binary>` - check that the binary does not have dylib dependencies outside of the allowed set (for portability).

## Running tests under debugger
- Launch codelldb under the debugger with `--multi-session --port=4711`.
- Run tests with the following environment variable set: `LLDB_SERVER=4711`.  For example: `LLDB_SERVER=4711 make check`.

## Useful targets:
- `dev_debugging` - build extension, adapter, debuggee and other stuff needed for debugging extension directly out of
   the build directory. After building this target you can run `code --extensionDevelopmentPath=${workspaceFolder}/build`
   to try out the extension.
- `adapter` - build the debug adapter.
- `extension` - build VSCode extension.
- `debuggee` - build debuggee subproject (used for testing).
- `tests` - build extension tests.
- `check` - run all tests.
- `vsix_bootstrap` - build VSIX package containing only the VSCode extension.
- `vsix_full` - build VSIX package including all required native binaries (for the current platform).
- `xclean` - extra-thorough cleaning.  Useful in cases of build problems caused by stale dependencies.
