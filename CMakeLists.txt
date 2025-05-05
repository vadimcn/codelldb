cmake_minimum_required(VERSION 3.10)
project(CodeLLDB)
enable_testing()

set(VERSION "1.11.5") # Base version

include(cmake/CopyFiles.cmake)

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_INSTALL_PREFIX $ENV{HOME}/.vscode/extensions/codelldb CACHE PATH "Install location")

set(VERSION_SUFFIX "-" CACHE INTERNAL "Version suffix")
if (VERSION_SUFFIX STREQUAL "-")
    string(TIMESTAMP VERSION_SUFFIX "-dev.%y%m%d%H%M" UTC)
endif()
set(VERSION "${VERSION}${VERSION_SUFFIX}")
message("Version ${VERSION}")

set(LLDB_PACKAGE $ENV{LLDB_PACKAGE} CACHE PATH "Zip archive containing LLDB files")
if (LLDB_PACKAGE)
    message("Using LLDB_PACKAGE=${LLDB_PACKAGE}")
else()
    message(FATAL_ERROR "LLDB_PACKAGE not set." )
endif()

if (CMAKE_SYSROOT)
    set(CMAKE_C_FLAGS "--sysroot=${CMAKE_SYSROOT} ${CMAKE_C_FLAGS}")
    set(CMAKE_CXX_FLAGS "--sysroot=${CMAKE_SYSROOT} ${CMAKE_CXX_FLAGS}")
endif()

set(TEST_TIMEOUT 5000 CACHE STRING "Test timeout [ms]")

set(WithEnv ${CMAKE_COMMAND} -E env)
set(UpdateFile ${CMAKE_COMMAND} -E copy_if_different)

# General OS-specific definitions
if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(Python python3)
    set(LibPrefix lib)
    set(LibSuffix .a)
    set(DylibSuffix .so)
    set(NPM npm)
    set(PathSep ":")
    set(DefaultTarget x86_64-unknown-linux-gnu)
    set(DefaultPlatformId linux-x64)
    set(AllowedDependencies linux-vdso.so.1 librt.so.1 libdl.so.2 libpthread.so.0 libgcc_s.so.1
                            libm.so.6 libc.so.6 libz.so.1 libutil.so.1
                            /lib64/ld-linux-x86-64.so.2)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    set(Python python3)
    set(LibPrefix lib)
    set(LibSuffix .a)
    set(DylibSuffix .dylib)
    set(SymSuffix .dSYM)
    set(NPM npm)
    set(PathSep ":")
    set(DefaultTarget x86_64-apple-darwin)
    set(DefaultPlatformId darwin-x64)
    set(AllowedDependencies Foundation CoreFoundation CoreServices Security SystemConfiguration
                            libcompression.dylib libz.1.dylib libSystem.B.dylib libc...1.dylib
                            libresolv.*.dylib libpmenergy.dylib libpmsample.dylib libobjc.A.dylib libiconv.*.dylib)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    set(Python python)
    set(ExeSuffix .exe)
    set(LibSuffix .lib)
    set(DylibSuffix .dll)
    set(SymSuffix .pdb)
    set(NPM npm.cmd)
    set(PathSep "$<SEMICOLON>")
    set(DefaultTarget x86_64-pc-windows-msvc)
    set(DefaultPlatformId win32-x64)
    set(AllowedDependencies api-ms-.* ws2_32.dll kernel32.dll ntdll.dll advapi32.dll user32.dll version.dll dbghelp.dll
                            psapi.dll oleaut32.dll shlwapi.dll rpcrt4.dll ole32.dll bcrypt.dll bcryptprimitives.dll
                            userenv.dll ucrtbase.dll vcruntime140.dll vcruntime140_1.dll msvcp140.dll)
else()
    message(FATAL_ERROR "${CMAKE_SYSTEM_NAME} target is not supported by this build script.")
endif()

string(JOIN "|" AllowedDependencies ${AllowedDependencies}) # Compose a regex

if (NOT LLVM_TRIPLE) # Normally set by cmake toolchain file
    set(LLVM_TRIPLE ${DefaultTarget})
endif()

if (NOT PLATFORM_ID)
    set(PLATFORM_ID ${DefaultPlatformId})
endif()

set(CargoFlags ${CargoFlags} --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml --target=${LLVM_TRIPLE})

if (CMAKE_BUILD_TYPE MATCHES Release|RelWithDebInfo)
    set(CargoFlags ${CargoFlags} --release)
    set(CargoBuildType release)
else()
    set(CargoBuildType debug)
endif()

# Adapter

add_subdirectory(adapter)
add_subdirectory(lldb)
add_subdirectory(lang_support)

add_dependencies(adapter lldb lang_support)

if (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    configure_file(cargo_config.windows.toml ${CMAKE_BINARY_DIR}/.cargo/config.toml)
else()
    configure_file(cargo_config.unix.toml ${CMAKE_BINARY_DIR}/.cargo/config.toml)
endif()

# Extension package content

set(PLATFORM_PACKAGE_URL "https://github.com/vadimcn/codelldb/releases/download/v\${version}/\${platformPackage}" CACHE STRING "")
configure_file(package.json ${CMAKE_CURRENT_BINARY_DIR}/package.json @ONLY)
configure_file(webpack.config.js ${CMAKE_CURRENT_BINARY_DIR}/webpack.config.js)
file(COPY ${CMAKE_CURRENT_SOURCE_DIR}/package-lock.json DESTINATION ${CMAKE_CURRENT_BINARY_DIR})

# Install node_modules
execute_process(
    COMMAND ${NPM} --loglevel verbose ci # like install, but actually respects package-lock file.
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    RESULT_VARIABLE Result
)
if (NOT ${Result} EQUAL 0)
    message(FATAL_ERROR "npm install failed: ${Result}")
endif()

# Resolve $refs
execute_process(
    COMMAND ${WithEnv} NODE_PATH=${CMAKE_CURRENT_BINARY_DIR}/node_modules node ${CMAKE_CURRENT_SOURCE_DIR}/tools/prep-package.js ${CMAKE_CURRENT_BINARY_DIR}/package.json ${CMAKE_CURRENT_BINARY_DIR}/package.json
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)

# Copy it back, so we can commit the lock file.
file(COPY ${CMAKE_CURRENT_BINARY_DIR}/package-lock.json DESTINATION ${CMAKE_CURRENT_SOURCE_DIR})

add_custom_target(update_lockfiles
    COMMAND ${UpdateFile} ${CMAKE_CURRENT_SOURCE_DIR}/package-lock.json ${CMAKE_CURRENT_BINARY_DIR}/package-lock.json
    COMMAND ${NPM} update
    COMMAND ${UpdateFile} ${CMAKE_CURRENT_BINARY_DIR}/package-lock.json ${CMAKE_CURRENT_SOURCE_DIR}/package-lock.json
    COMMAND cargo update
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)

# Extension and tests

if (CMAKE_BUILD_TYPE MATCHES Release|RelWithDebInfo)
    set(WebpackMode production)
else()
    set(WebpackMode development)
endif()

file(GLOB_RECURSE ExtensionFiles ${CMAKE_CURRENT_SOURCE_DIR}/extension/*.ts ${CMAKE_CURRENT_SOURCE_DIR}/extension/*.json)
add_custom_command(
    OUTPUT extension.js
    DEPENDS ${ExtensionFiles}
    COMMAND ${NPM} run webpack -- --mode=${WebpackMode} --stats=minimal --output-path=${CMAKE_CURRENT_BINARY_DIR} --output-filename=extension.js
        ${CMAKE_CURRENT_SOURCE_DIR}/extension/main.ts
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)
add_custom_target(extension
    DEPENDS extension.js
)

file(GLOB_RECURSE TestFiles ${CMAKE_CURRENT_SOURCE_DIR}/tests/*.ts ${CMAKE_CURRENT_SOURCE_DIR}/tests/*.json)
add_custom_command(
    OUTPUT tests.js
    DEPENDS ${TestFiles} ${ExtensionFiles}
    COMMAND ${NPM} run webpack -- --mode=${WebpackMode} --stats=minimal --output-path=${CMAKE_CURRENT_BINARY_DIR} --output-filename=tests.js
        ${CMAKE_CURRENT_SOURCE_DIR}/tests/adapter.test.ts
        ${CMAKE_CURRENT_SOURCE_DIR}/tests/util.test.ts
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)
add_custom_target(adapter_tests
    DEPENDS tests.js
)

# Extension package resources

add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/README.md ${CMAKE_CURRENT_BINARY_DIR}/README.md)
add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/CHANGELOG.md ${CMAKE_CURRENT_BINARY_DIR}/CHANGELOG.md)
add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/images/lldb.png ${CMAKE_CURRENT_BINARY_DIR}/images/lldb.png)
add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/images/user.svg ${CMAKE_CURRENT_BINARY_DIR}/images/user.svg)
add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/images/users.svg ${CMAKE_CURRENT_BINARY_DIR}/images/users.svg)
add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/syntaxes/disassembly.json ${CMAKE_CURRENT_BINARY_DIR}/syntaxes/disassembly.json)

# For extension debugging

add_custom_target(dev_debugging
    DEPENDS adapter debuggee extension ${PackageFiles}
)

# VSIX packages

set(PackagedFilesBootstrap
    README.md
    CHANGELOG.md
    extension.js
    images/*
    syntaxes/*
)

set(PackagedFilesFull
    ${PackagedFilesBootstrap}
    platform.ok
    adapter/codelldb
    adapter/codelldb.exe
    adapter/*.so
    adapter/*.dylib
    adapter/*.dll
    adapter/scripts/**/*.py
    lldb/bin/**/*
    lldb/lib/**/*
    lldb/DLLs/**/*
    lang_support/**/*.py
)

# Indicator file used to determine whether we have the platform package.
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/platform.ok "")

set(Content **)
foreach(Line ${PackagedFilesBootstrap})
    set(Content ${Content}\n!${Line})
endforeach()
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-bootstrap ${Content})

set(Content **)
foreach(Line ${PackagedFilesFull})
    set(Content ${Content}\n!${Line})
endforeach()
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-full ${Content})

set(VsceOptions
    --baseContentUrl https://github.com/vadimcn/codelldb/blob/v${VERSION}
    --baseImagesUrl https://github.com/vadimcn/codelldb/raw/v${VERSION}
)

add_custom_target(vsix_bootstrap
    DEPENDS extension ${PackageFiles}
    COMMAND ${UpdateFile} ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-bootstrap ${CMAKE_CURRENT_BINARY_DIR}/.vscodeignore
    COMMAND echo Y | ${NPM} run vsce -- package ${VsceOptions} -o ${CMAKE_CURRENT_BINARY_DIR}/codelldb-bootstrap.vsix
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)

add_custom_target(vsix_full
    DEPENDS extension lldb adapter ${PackageFiles}
    COMMAND ${UpdateFile} ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-full ${CMAKE_CURRENT_BINARY_DIR}/.vscodeignore
    COMMAND echo Y | ${NPM} run vsce -- package ${VsceOptions} --target ${PLATFORM_ID} -o ${CMAKE_CURRENT_BINARY_DIR}/codelldb-full.vsix
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)

# Strip native binaries if building vsix_full

if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    if (NOT CMAKE_STRIP)
        set(CMAKE_STRIP strip)
    endif()

    add_custom_target(strip_binaries
        DEPENDS lldb adapter
        COMMAND find ${CMAKE_CURRENT_BINARY_DIR}/adapter -type f "\\(" -executable -o -name "*.so" -o -name "*.so.*" "\\)" -print -exec ${CMAKE_STRIP} --strip-debug "{}" "\;"
        COMMAND find ${CMAKE_CURRENT_BINARY_DIR}/lldb    -type f "\\(" -executable -o -name "*.so" -o -name "*.so.*" "\\)" -not -name "*.py" -print -exec ${CMAKE_STRIP} --strip-debug "{}" "\;"
        COMMENT "Stripping debug info"
        USES_TERMINAL
    )
    add_dependencies(vsix_full strip_binaries)
endif()

# Build both VSIX packages, then extract vsix_bootstrap to build/extracted

add_custom_target(vsix_extracted
    DEPENDS vsix_bootstrap vsix_full
    COMMAND rm -rf ${CMAKE_CURRENT_BINARY_DIR}/extracted
    COMMAND unzip -o ${CMAKE_CURRENT_BINARY_DIR}/codelldb-full.vsix -d ${CMAKE_CURRENT_BINARY_DIR}/extracted
    COMMAND ${UpdateFile} ${CMAKE_CURRENT_BINARY_DIR}/extension/*.map ${CMAKE_CURRENT_BINARY_DIR}/extracted/extension/extension
)

# Debuggee

if (NOT ${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    message("Configuring debuggee")
    file(MAKE_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee)
    execute_process(
        COMMAND ${CMAKE_COMMAND} ${CMAKE_SOURCE_DIR}/debuggee -G${CMAKE_GENERATOR} -DCMAKE_TOOLCHAIN_FILE=${CMAKE_TOOLCHAIN_FILE} -DCMAKE_SYSROOT=${CMAKE_SYSROOT}
        WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee
    )
    add_custom_target(debuggee
        COMMAND ${CMAKE_COMMAND} --build ${CMAKE_CURRENT_BINARY_DIR}/debuggee
    )
else()
    # On Windows we want to check both GNU DWARF and MSVC PDB debug info kinds.
    message("Configuring debuggee-gnu")
    file(MAKE_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee-gnu)
    execute_process(
        COMMAND ${CMAKE_COMMAND} ${CMAKE_SOURCE_DIR}/debuggee -G${CMAKE_GENERATOR} -DCMAKE_TOOLCHAIN_FILE=${CMAKE_SOURCE_DIR}/cmake/toolchain-x86_64-windows-gnu.cmake
        WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee-gnu
    )
    message("Configuring debuggee-msvc")
    file(MAKE_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee-msvc)
    execute_process(
        COMMAND ${CMAKE_COMMAND} ${CMAKE_SOURCE_DIR}/debuggee -G${CMAKE_GENERATOR} -DCMAKE_TOOLCHAIN_FILE=${CMAKE_SOURCE_DIR}/cmake/toolchain-x86_64-windows-msvc.cmake
        WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee-msvc
    )
    add_custom_target(debuggee
        COMMAND ${CMAKE_COMMAND} --build ${CMAKE_CURRENT_BINARY_DIR}/debuggee-gnu
        COMMAND ${CMAKE_COMMAND} --build ${CMAKE_CURRENT_BINARY_DIR}/debuggee-msvc
    )
endif()

# Tests and test targets

set(MochaTest ${NPM} run mocha --
    --ui tdd --timeout ${TEST_TIMEOUT} --exit
    --require source-map-support/register
    --require mocha-suppress-logs
    #-g "rust_variables"
    ${CMAKE_BINARY_DIR}/tests.js
)

if (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    set(TestTriples x86_64-pc-windows-gnu x86_64-pc-windows-msvc)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    set(TestTriples x86_64-apple-darwin)
else()
    set(TestTriples x86_64-unknown-linux-gnu)
endif()

foreach(TestTriple ${TestTriples})
    set(TestName adapter:${TestTriple})
    add_test(NAME ${TestName}
        COMMAND ${WithEnv} TARGET_TRIPLE=${TestTriple} NODE_PATH=${CMAKE_BINARY_DIR} BUILD_DIR=${CMAKE_BINARY_DIR} SOURCE_DIR=${CMAKE_SOURCE_DIR} ${MochaTest}
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR})
    set_property(TEST ${TestName} PROPERTY RUN_SERIAL TRUE)
endforeach(TestTriple)

# Build testing pre-requisites
add_custom_target(tests
    DEPENDS lldb adapter debuggee adapter_tests
)

# Run all tests
add_custom_target(check
    DEPENDS tests
    COMMAND ${CMAKE_CTEST_COMMAND} --verbose
    USES_TERMINAL
)

# Install

install(CODE "file(REMOVE_RECURSE \"${CMAKE_INSTALL_PREFIX}/codelldb\")")
install(
    DIRECTORY ${ExtensionRoot}
    DESTINATION .
)

# XtraClean

add_custom_target(xclean
    COMMAND ${CMAKE_COMMAND} --build ${CMAKE_BINARY_DIR} --target clean
    COMMAND ${CMAKE_COMMAND} -E remove_directory CMakeFiles
    COMMAND ${CMAKE_COMMAND} -E remove_directory adapter
    COMMAND ${CMAKE_COMMAND} -E remove_directory debuggee
    COMMAND ${CMAKE_COMMAND} -E remove_directory extension
    COMMAND ${CMAKE_COMMAND} -E remove_directory lldb
    COMMAND ${CMAKE_COMMAND} -E remove_directory node_modules
    COMMAND ${CMAKE_COMMAND} -E remove_directory target
    COMMAND ${CMAKE_COMMAND} -E remove_directory tests
    COMMAND ${CMAKE_COMMAND} -E remove_directory tmp
    COMMAND ${CMAKE_COMMAND} ..
)
