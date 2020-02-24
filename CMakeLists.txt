cmake_minimum_required(VERSION 3.10)
project(CodeLLDB)
enable_testing()

set(VERSION 1.5.0)

include(cmake/CopyFiles.cmake)

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_INSTALL_PREFIX $ENV{HOME}/.vscode/extensions/vscode-lldb CACHE PATH "Install location")

set(LLDB_ROOT $ENV{LLDB_ROOT} CACHE PATH "Root of LLDB build directory")
if (LLDB_ROOT)
    message("Using LLDB from ${LLDB_ROOT}")
else()
    message(FATAL_ERROR "LLDB_ROOT not set." )
endif()

# General OS-specific definitions
if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(DylibPrefix lib)
    set(DylibSuffix .so)
    set(NPM npm)
    set(PathSep ":")
    set(DefaultTarget x86_64-unknown-linux-gnu)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    set(DylibPrefix lib)
    set(DylibSuffix .dylib)
    set(SymSuffix .dSYM)
    set(NPM npm)
    set(PathSep ":")
    set(DefaultTarget x86_64-apple-darwin)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    set(ExeSuffix .exe)
    set(DylibSuffix .dll)
    set(SymSuffix .pdb)
    set(NPM npm.cmd)
    set(PathSep "$<SEMICOLON>")
    set(DefaultTarget x86_64-pc-windows-msvc)
else()
    message(FATAL_ERROR "${CMAKE_SYSTEM_NAME} target is not supported by this build script.")
endif()


if (NOT LLVM_TRIPLE)
set(LLVM_TRIPLE ${DefaultTarget})
endif()

set(WithEnv ${CMAKE_COMMAND} -E env)
set(UpdateFile ${CMAKE_COMMAND} -E copy_if_different)

add_subdirectory(adapter2)
add_subdirectory(lldb)

# Extension package content

set(PLATFORM_PACKAGE_URL "https://github.com/vadimcn/vscode-lldb/releases/download/v\${version}/\${platformPackage}" CACHE STRING "")
configure_file(package.json ${CMAKE_CURRENT_BINARY_DIR}/package.json @ONLY)
configure_file(webpack.config.js ${CMAKE_CURRENT_BINARY_DIR}/webpack.config.js @ONLY)
file(COPY ${CMAKE_CURRENT_SOURCE_DIR}/package-lock.json DESTINATION ${CMAKE_CURRENT_BINARY_DIR})

# Run 'npm install'
execute_process(
    COMMAND ${NPM} install
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    RESULT_VARIABLE Result
)
if (NOT ${Result} EQUAL 0)
    message(FATAL_ERROR "npm intall failed: ${Result}")
endif()

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

add_custom_target(extension
    COMMAND
    COMMAND ${NPM} run webpack -- --display=minimal --mode=${WebpackMode} --output=${CMAKE_CURRENT_BINARY_DIR}/extension.js
        ${CMAKE_CURRENT_SOURCE_DIR}/extension/main.ts
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)

add_custom_target(tests
    COMMAND ${NPM} run webpack -- --display=minimal --mode=${WebpackMode} --output=${CMAKE_CURRENT_BINARY_DIR}/tests.js
        ${CMAKE_CURRENT_SOURCE_DIR}/tests/adapter.test.ts
        ${CMAKE_CURRENT_SOURCE_DIR}/tests/util.test.ts
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)

# Extension package resources

add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/README.md ${CMAKE_CURRENT_BINARY_DIR}/README.md)
add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/CHANGELOG.md ${CMAKE_CURRENT_BINARY_DIR}/CHANGELOG.md)
add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/images/lldb.png ${CMAKE_CURRENT_BINARY_DIR}/images/lldb.png)
add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/images/checked.svg ${CMAKE_CURRENT_BINARY_DIR}/images/checked.svg)
add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/images/unchecked.svg ${CMAKE_CURRENT_BINARY_DIR}/images/unchecked.svg)
add_copy_file(PackageFiles ${CMAKE_CURRENT_SOURCE_DIR}/syntaxes/disassembly.json ${CMAKE_CURRENT_BINARY_DIR}/syntaxes/disassembly.json)

# For extension debugging

add_custom_target(dev_debugging
    DEPENDS codelldb debuggee extension ${PackageFiles}
)

# VSIX packages

set(PackagedFilesPortable
    README.md
    CHANGELOG.md
    extension.js
    formatters/*
    images/*
    syntaxes/*
)

set(PackagedFilesFull
    ${PackagedFilesPortable}
    platform.ok
    adapter2/*.py
    adapter2/codelldb
    adapter2/libcodelldb.so
    adapter2/libcodelldb_python.so
    adapter2/libcodelldb.dylib
    adapter2/libcodelldb_python.dylib
    adapter2/codelldb.exe
    adapter2/codelldb.dll
    adapter2/codelldb_python.dll
    lldb/bin/**/*
    lldb/lib/**/*
)

# Indicator file used to determine whether we have the platform package.
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/platform.ok "")

set(Content **)
foreach(Line ${PackagedFilesPortable})
    set(Content ${Content}\n!${Line})
endforeach()
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-portable ${Content})

set(Content **)
foreach(Line ${PackagedFilesFull})
    set(Content ${Content}\n!${Line})
endforeach()
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-full ${Content})

set(VsceOptions
    --baseContentUrl https://github.com/vadimcn/vscode-lldb/blob/v${VERSION}
    --baseImagesUrl https://github.com/vadimcn/vscode-lldb/raw/v${VERSION}
)

add_custom_target(vsix_portable
    DEPENDS extension ${PackageFiles}
    COMMAND ${UpdateFile} ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-portable ${CMAKE_CURRENT_BINARY_DIR}/.vscodeignore
    COMMAND ${NPM} run vsce -- package ${VsceOptions} -o ${CMAKE_CURRENT_BINARY_DIR}/codelldb-portable.vsix
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)

add_custom_target(vsix_full
    DEPENDS extension lldb codelldb ${PackageFiles}
    COMMAND ${UpdateFile} ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-full ${CMAKE_CURRENT_BINARY_DIR}/.vscodeignore
    COMMAND ${NPM} run vsce -- package ${VsceOptions} -o ${CMAKE_CURRENT_BINARY_DIR}/codelldb-full.vsix
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)

# Strip native binaries if building vsix_full

if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    if (NOT CMAKE_STRIP)
        set(CMAKE_STRIP strip)
    endif()

    add_custom_target(strip_binaries
        DEPENDS lldb codelldb
        COMMAND find ${CMAKE_CURRENT_BINARY_DIR}/adapter2 -type f "\\(" -executable -o -name "*.so" -o -name "*.so.*" "\\)" -print -exec ${CMAKE_STRIP} --strip-debug "{}" "\;"
        COMMAND find ${CMAKE_CURRENT_BINARY_DIR}/lldb     -type f "\\(" -executable -o -name "*.so" -o -name "*.so.*" "\\)" -not -name "*.py" -print -exec ${CMAKE_STRIP} --strip-debug "{}" "\;"
        COMMENT "Stripping debug info"
        USES_TERMINAL
    )
    add_dependencies(vsix_full strip_binaries)
endif()

# Build both VSIX packages, then extract vsix_portable to build/extracted

add_custom_target(vsix_extracted
    DEPENDS vsix_portable vsix_full
    COMMAND rm -rf ${CMAKE_CURRENT_BINARY_DIR}/extracted
    COMMAND unzip -o ${CMAKE_CURRENT_BINARY_DIR}/codelldb-full.vsix -d ${CMAKE_CURRENT_BINARY_DIR}/extracted
    COMMAND ${UpdateFile} ${CMAKE_CURRENT_BINARY_DIR}/extension/*.map ${CMAKE_CURRENT_BINARY_DIR}/extracted/extension/extension
)

# Debuggee

if (NOT ${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    file(MAKE_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee)
    execute_process(
        COMMAND ${CMAKE_COMMAND} ${CMAKE_SOURCE_DIR}/debuggee -G "${CMAKE_GENERATOR}"
        WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee
    )
    add_custom_target(debuggee
        COMMAND ${CMAKE_COMMAND} --build ${CMAKE_CURRENT_BINARY_DIR}/debuggee
    )
else()
    # On Windows we want to check both MSVC PDB and GNU DWARF debug info kinds.
    file(MAKE_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee-msvc)
    execute_process(
        COMMAND ${CMAKE_COMMAND} ${CMAKE_SOURCE_DIR}/debuggee -G "${CMAKE_GENERATOR}" -DCMAKE_C_COMPILER=cl -DCMAKE_CXX_COMPILER=cl
        WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee-msvc
    )
    file(MAKE_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee-gnu)
    execute_process(
        COMMAND ${CMAKE_COMMAND} ${CMAKE_SOURCE_DIR}/debuggee -G "${CMAKE_GENERATOR}" -DCMAKE_C_COMPILER=gcc -DCMAKE_CXX_COMPILER=g++
        WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/debuggee-gnu
    )
    add_custom_target(debuggee
        COMMAND ${CMAKE_COMMAND} --build ${CMAKE_CURRENT_BINARY_DIR}/debuggee-msvc
        COMMAND ${CMAKE_COMMAND} --build ${CMAKE_CURRENT_BINARY_DIR}/debuggee-gnu
    )
endif()

# Tests

set(MochaTest ${NPM} run mocha --
    -u tdd --timeout 30000 --exit
    --require source-map-support/register
    #-g "rust_variables"
    ${CMAKE_BINARY_DIR}/tests.js
)

# Build everything for testing
add_dependencies(tests codelldb debuggee)

# Tests and test targets

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

add_custom_target(check
    DEPENDS tests cargo_test
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure
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
    COMMAND ${CMAKE_COMMAND} -E remove_directory adapter2
    COMMAND ${CMAKE_COMMAND} -E remove_directory debuggee
    COMMAND ${CMAKE_COMMAND} -E remove_directory extension
    COMMAND ${CMAKE_COMMAND} -E remove_directory lldb
    COMMAND ${CMAKE_COMMAND} -E remove_directory node_modules
    COMMAND ${CMAKE_COMMAND} -E remove_directory target
    COMMAND ${CMAKE_COMMAND} -E remove_directory tests
    COMMAND ${CMAKE_COMMAND} -E remove_directory tmp
    COMMAND ${CMAKE_COMMAND} ..
)
