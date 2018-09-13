cmake_minimum_required(VERSION 3.10)

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
else()
    set(ZipSymlinks -y)
endif()

add_subdirectory(adapter2)
add_subdirectory(lldb)
add_subdirectory(extension)
add_subdirectory(tests)

# Classic adapter

add_custom_target(adapter ALL
    DEPENDS lldb
)
add_copy_files(adapter
    DESTINATION ${CMAKE_CURRENT_BINARY_DIR}/adapter
    GLOB ${CMAKE_SOURCE_DIR}/adapter/*.py
)
add_copy_files(adapter
    DESTINATION ${CMAKE_CURRENT_BINARY_DIR}/formatters
    GLOB ${CMAKE_SOURCE_DIR}/formatters/*.py
)

# Misc files for extension package

add_copy_files(package_files
    DESTINATION ${CMAKE_CURRENT_BINARY_DIR}
    FILES package.json README.md MANUAL.md CHANGELOG.md
)
add_copy_files(package_files
    DESTINATION ${CMAKE_CURRENT_BINARY_DIR}/images
    GLOB ${CMAKE_SOURCE_DIR}/images/*
)
add_copy_files(package_files
    DESTINATION ${CMAKE_CURRENT_BINARY_DIR}/syntaxes
    GLOB ${CMAKE_SOURCE_DIR}/syntaxes/*
)

# For extension debugging

add_custom_target(debugging DEPENDS adapter codelldb debuggee extension package_files)

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

add_copy_directory(node_modules
    ${CMAKE_SOURCE_DIR}/node_modules/yauzl
    DESTINATION  ${CMAKE_BINARY_DIR}/node_modules/yauzl
)
add_copy_directory(node_modules
    ${CMAKE_SOURCE_DIR}/node_modules/buffer-crc32
    DESTINATION  ${CMAKE_BINARY_DIR}/node_modules/buffer-crc32
)
add_copy_directory(node_modules
    ${CMAKE_SOURCE_DIR}/node_modules/fd-slicer
    DESTINATION  ${CMAKE_BINARY_DIR}/node_modules/fd-slicer
)
add_copy_directory(node_modules
    ${CMAKE_SOURCE_DIR}/node_modules/pend
    DESTINATION  ${CMAKE_BINARY_DIR}/node_modules/pend
)

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
    DEPENDS adapter extension node_modules package_files
    COMMAND ${CMAKE_COMMAND} -E copy ${CMAKE_CURRENT_BINARY_DIR}/vscodeignore-portable ${CMAKE_CURRENT_BINARY_DIR}/.vscodeignore
    COMMAND vsce package -o ${CMAKE_CURRENT_BINARY_DIR}/vscode-lldb-portable.vsix
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
)

add_custom_target(vsix_full
    DEPENDS adapter extension node_modules package_files lldb codelldb
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

set(MochaCommand "${CMAKE_SOURCE_DIR}/node_modules/.bin/mocha${NodeProgExt}"
    -u tdd
    --timeout 30000
    --exit
    --require source-map-support/register
    #-g "to the end"
    ${CMAKE_BINARY_DIR}/tests/tests
)

if (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
    set(TestAdapters bundled native)
    set(TestTriples x86_64-pc-windows-gnu x86_64-pc-windows-msvc)
else()
    set(TestAdapters classic bundled native)
    set(TestTriples x86_64-unknown-linux-gnu)
endif()

foreach(TestTriple ${TestTriples})
    foreach(AdapterType ${TestAdapters})
        add_test(NAME ${AdapterType}:${TestTriple}
            COMMAND ${CMAKE_COMMAND} -E env ADAPTER_TYPE=${AdapterType} TARGET_TRIPLE=${TestTriple} ${MochaCommand}
            WORKING_DIRECTORY ${CMAKE_SOURCE_DIR})
    endforeach(AdapterType)
endforeach(TestTriple)

# Build everything for testing
add_dependencies(tests adapter codelldb debuggee)

# Run tests
add_custom_target(check DEPENDS tests
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure
)

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
