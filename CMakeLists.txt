cmake_minimum_required(VERSION 3.10)

set(VERSION 1.2.3-dev)

include(cmake/CopyFiles.cmake)
include(cmake/Typescript.cmake)

set(LLDB_ROOT $ENV{LLDB_ROOT} CACHE PATH "Root of LLDB build directory")
if (NOT LLDB_ROOT)
    message(FATAL_ERROR "LLDB_ROOT not set." )
endif()

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_INSTALL_PREFIX $ENV{HOME}/.vscode/extensions/vscode-lldb CACHE PATH "Install location")

if (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
    set(NodeProgExt .cmd)
endif()

add_subdirectory(adapter2)
add_subdirectory(lldb)
add_subdirectory(extension)
add_subdirectory(tests)

# Extension package content

configure_file(package.json ${CMAKE_CURRENT_BINARY_DIR}/package.json @ONLY)
configure_file(README.md ${CMAKE_CURRENT_BINARY_DIR}/README.md @ONLY)
configure_file(MANUAL.md ${CMAKE_CURRENT_BINARY_DIR}/MANUAL.md @ONLY)
configure_file(CHANGELOG.md ${CMAKE_CURRENT_BINARY_DIR}/CHANGELOG.md @ONLY)
configure_file(images/lldb.png ${CMAKE_CURRENT_BINARY_DIR}/images/lldb.png COPYONLY)
configure_file(images/disasm.png ${CMAKE_CURRENT_BINARY_DIR}/images/disasm.png COPYONLY)
configure_file(images/plotting.png ${CMAKE_CURRENT_BINARY_DIR}/images/plotting.png COPYONLY)
configure_file(images/source.png ${CMAKE_CURRENT_BINARY_DIR}/images/source.png COPYONLY)
configure_file(syntaxes/disassembly.json ${CMAKE_CURRENT_BINARY_DIR}/syntaxes/disassembly.json COPYONLY)

execute_process(
    COMMAND npm${NodeProgExt} install
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    RESULT_VARIABLE Result
)
if (NOT ${Result} EQUAL 0)
    message(FATAL_ERROR "npm intall failed: ${Result}")
endif()

# Classic adapter

add_custom_target(adapter ALL
    DEPENDS lldb
)
file(GLOB_RECURSE AdapterFiles
    LIST_DIRECTORIES false
    ${CMAKE_SOURCE_DIR}/adapter/*.py
)
add_copy_files_to(adapter ${CMAKE_BINARY_DIR}/adapter ${AdapterFiles})
file(GLOB_RECURSE AdapterFiles
    LIST_DIRECTORIES false
    ${CMAKE_SOURCE_DIR}/formatters/*.py
)
add_copy_files_to(adapter ${CMAKE_BINARY_DIR}/formatters ${AdapterFiles})


# For extension debugging

add_custom_target(dev_debugging DEPENDS adapter codelldb debuggee extension)

# Strip native binaries

if (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Linux")
    set(StripBinaries strip
                   ${CMAKE_CURRENT_BINARY_DIR}/adapter2/codelldb
                   ${CMAKE_CURRENT_BINARY_DIR}/adapter2/libcodelldb.so
                   ${CMAKE_CURRENT_BINARY_DIR}/lldb/bin/lldb
                   ${CMAKE_CURRENT_BINARY_DIR}/lldb/bin/lldb-server
                   ${CMAKE_CURRENT_BINARY_DIR}/lldb/lib/liblldb.so.8-custom
                   ${CMAKE_CURRENT_BINARY_DIR}/lldb/lib/python2.7/site-packages/lldb/lldb-argdumper
                   )
endif()

# VSIX packages


set(PackagedFilesPortable
    README.md
    CHANGELOG.md
    MANUAL.md
    adapter/**/*.py
    formatters/*
    images/*
    syntaxes/*
    extension/*.js
    node_modules/**/*.js
)

set(PackagedFilesFull
    ${PackagedFilesPortable}
    adapter2/*.py
    adapter2/codelldb
    adapter2/libcodelldb.so
    adapter2/libcodelldb.dylib
    adapter2/codelldb.exe
    adapter2/codelldb.dll
    lldb/bin/**/*
    lldb/lib/**/*
)

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

add_custom_target(vsix_portable
    DEPENDS adapter extension
    COMMAND ${CMAKE_COMMAND} -E copy ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-portable ${CMAKE_CURRENT_BINARY_DIR}/.vscodeignore
    COMMAND vsce package -o ${CMAKE_CURRENT_BINARY_DIR}/vscode-lldb-portable.vsix
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
)

add_custom_target(vsix_full
    DEPENDS adapter extension lldb codelldb
    COMMAND ${StripBinaries}
    COMMAND ${CMAKE_COMMAND} -E copy ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-full ${CMAKE_CURRENT_BINARY_DIR}/.vscodeignore
    COMMAND vsce package -o ${CMAKE_CURRENT_BINARY_DIR}/vscode-lldb-full.vsix
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
)

# Build vsix_portable, then extract to tmp

add_custom_target(vsix_extracted
    DEPENDS vsix_portable
    COMMAND rm -rf ${CMAKE_CURRENT_BINARY_DIR}/tmp
    COMMAND unzip -o ${CMAKE_CURRENT_BINARY_DIR}/vscode-lldb-portable.vsix -d ${CMAKE_CURRENT_BINARY_DIR}/tmp
)

# Debuggee

if (NOT ${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
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

enable_testing()

set(MochaCommand "${CMAKE_BINARY_DIR}/node_modules/.bin/mocha${NodeProgExt}"
    -u tdd
    --timeout 30000
    --exit
    --require source-map-support/register
    #-g "stop on a breakpoint 3"
    ${CMAKE_BINARY_DIR}/tests
)

# Build everything for testing
add_dependencies(tests adapter codelldb debuggee)

# Tests and test targets
add_custom_target(check)

if (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
    set(TestAdapters bundled native)
    set(TestTriples x86_64-pc-windows-gnu x86_64-pc-windows-msvc)
elseif (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Darwin")
    set(TestAdapters classic bundled native)
    set(TestTriples x86_64-apple-darwin)
else()
    set(TestAdapters classic bundled native)
    set(TestTriples x86_64-unknown-linux-gnu)
endif()

foreach(AdapterType ${TestAdapters})
    foreach(TestTriple ${TestTriples})
        add_test(NAME ${AdapterType}:${TestTriple}
            COMMAND ${CMAKE_COMMAND} -E env ADAPTER_TYPE=${AdapterType} TARGET_TRIPLE=${TestTriple} NODE_PATH=${CMAKE_BINARY_DIR} ${MochaCommand}
            WORKING_DIRECTORY ${CMAKE_SOURCE_DIR})
    endforeach(TestTriple)

    add_custom_target(check_${AdapterType} DEPENDS tests
        COMMAND ${CMAKE_CTEST_COMMAND} -R ${AdapterType} --output-on-failure
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
        USES_TERMINAL
    )
    add_dependencies(check check_${AdapterType})
endforeach(AdapterType)

# Install

install(CODE "file(REMOVE_RECURSE \"${CMAKE_INSTALL_PREFIX}/vscode-lldb\")")
install(
    DIRECTORY ${ExtensionRoot}
    DESTINATION .
)

# XtraClean

add_custom_target(xclean
    COMMAND ${CMAKE_COMMAND} -E remove_directory adapter
    COMMAND ${CMAKE_COMMAND} -E remove_directory adapter2
    COMMAND ${CMAKE_COMMAND} -E remove_directory lldb
    COMMAND ${CMAKE_COMMAND} -E remove_directory extension
    COMMAND ${CMAKE_COMMAND} -E remove_directory tests
    COMMAND ${CMAKE_COMMAND} -E remove_directory tmp
    COMMAND ${CMAKE_COMMAND} -E remove_directory debuggee
    COMMAND ${CMAKE_COMMAND} -E remove_directory node_modules
    COMMAND ${CMAKE_COMMAND} ..
)
