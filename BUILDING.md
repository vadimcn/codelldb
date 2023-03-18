# Prerequisites
- Node.js and npm, v10 or later.
- Python 3.5 or later.
- C++ compiler (GCC, clang or MSVC).
- Rust 1.61 or later.
- (Windows only) mingw-w64 toolchain (used for tests).

# Clone the repo
```
git clone <from> codelldb
cd codelldb
```

# Create LLDB package

This is a zip archive containing the subset of LLDB files which CodeLLDB needs at runtime.

Building a minimal self-contained LLDB package is rather tricky, I recommend just zipping up the contents of
`<HOME>/.vscode/extensions/vadimcn.vscode-lldb-<version>/lldb` directory from an existing CodeLLDB installation.

If you choose to build LLVM from source, it is usually sufficient to include in the archive `bin` and `lib`
subdirectories from the build output.

# Configure

- On Windows, you should do this from "x64 Native Tools Command Prompt", so that MSVC environment is set up correctly.
- On Mac, I recommend starting a new shell via `xcrun --sdk macosx zsh`

```
cd codelldb
mkdir build  # (the build directory may be changed, but tasks.json assumes it's "build" under the project root)
cd build
cmake .. -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-x86_64-linux-gnu.cmake -DLLDB_PACKAGE=<path to zip archive created in the previous step>
```
If you are on some other platform, edit the toolchain file accordingly. You *will* get linker errors if you don't use the toolchain file

# VSCode
If you intend to run and debug tests in VSCode, you may want to create a symlink from `<souce>/.cargo/config.toml`
to `<build>/.cargo/config.toml`.

# Build
```
cd build
make <target>
```

## Useful targets:
- `dev_debugging` - build extension, adapter, debuggee and other stuff needed for debugging extension directly out of
   the build directory. After building this target you can run `code --extensionDevelopmentPath=${workspaceFolder}/build`
   to try out the extension.
- `adapter` - build the debug adapter.
- `extension` - build VSCode extension.
- `debuggee` - build debuggee subproject (used for testing).
- `cargo_test` - run `cargo test` on all codelldb crates.
- `tests` - build extension tests.
- `check` - run all tests.
- `vsix_bootstrap` - build VSIX package containing only the VSCode extension.
- `vsix_full` - build VSIX package including all required native binaries (for the current platform).
- `xclean` - extra-thorough cleaning.  Useful in cases of build problems caused by stale dependencies.
