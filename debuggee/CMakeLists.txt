cmake_minimum_required(VERSION 3.10)
project(debuggee)

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_BUILD_TYPE Debug)

# Shared [lib]debuggee2.[so|dylib|dll]
add_library(debuggee2 SHARED
    cpp/libdebuggee/libmain.cpp
)
# debuggee
add_executable(debuggee
    cpp/debuggee.cpp
    cpp/no_line_info.cpp # without debug info, for testing disassembly
    cpp/denorm_path.cpp  # test debugger's path normalization
    cpp/remote1/remote_path.cpp  # test source map
    cpp/remote2/remote_path.cpp  # test source map
)

if (NOT ${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
    target_link_libraries(debuggee dl pthread)
else()
    set(Drive "C:")
endif()

if ("${CMAKE_CXX_COMPILER_ID}" STREQUAL "MSVC")
    set_source_files_properties(cpp/no_line_info.cpp PROPERTIES COMPILE_FLAGS /DEBUG:None)
else()
    set_source_files_properties(cpp/no_line_info.cpp PROPERTIES COMPILE_FLAGS -g0)
    set_source_files_properties(cpp/denorm_path.cpp PROPERTIES COMPILE_FLAGS -fdebug-prefix-map=${CMAKE_SOURCE_DIR}/cpp=../../debuggee/cpp/../cpp/.)
    set_source_files_properties(cpp/remote1/remote_path.cpp PROPERTIES COMPILE_FLAGS -fdebug-prefix-map=${CMAKE_SOURCE_DIR}/cpp/remote1=${Drive}/remote1)
    set_source_files_properties(cpp/remote2/remote_path.cpp PROPERTIES COMPILE_FLAGS -fdebug-prefix-map=${CMAKE_SOURCE_DIR}/cpp/remote2=${Drive}/remote2)
endif()

# rusttypes
if (NOT ${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
    set(Rustc rustc)
    set(Rusttypes rusttypes)
elseif ("${CMAKE_CXX_COMPILER_ID}" STREQUAL "MSVC")
    set(Rustc rustup run nightly-x86_64-pc-windows-msvc rustc)
    set(Rusttypes rusttypes.exe)
else()
    set(Rustc rustup run nightly-x86_64-pc-windows-gnu rustc)
    set(Rusttypes rusttypes.exe)
endif()

add_custom_target(rusttypes
    COMMAND ${Rustc} ${CMAKE_CURRENT_SOURCE_DIR}/rust/types.rs -g -o ${Rusttypes}
)

add_dependencies(debuggee debuggee2 rusttypes)
