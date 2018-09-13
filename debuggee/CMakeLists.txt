cmake_minimum_required(VERSION 3.10)
project(debuggee)

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_BUILD_TYPE Debug)

# Shared [lib]debuggee2.[so|dylib|dll]
add_library(debuggee2 SHARED
    cpp/libdebuggee/libmain.cpp
)

# without debug info, for testing disassembly
add_library(debuggee_nodebinfo STATIC
    cpp/no_line_info.cpp
)
if ("${CMAKE_CXX_COMPILER_ID}" STREQUAL "MSVC")
    target_compile_options(debuggee_nodebinfo PRIVATE /DEBUG:None)
else()
    target_compile_options(debuggee_nodebinfo PRIVATE -g0)
endif()

# debuggee
add_executable(debuggee
    cpp/debuggee.cpp
)

if (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
    target_link_libraries(debuggee debuggee_nodebinfo)
else()
    # Use unnormalized path on purpose - to test debugger's path normalization
    target_compile_options(debuggee PRIVATE -fdebug-prefix-map=${CMAKE_SOURCE_DIR}/cpp=../../debuggee/cpp/../cpp/.)
    target_link_libraries(debuggee debuggee_nodebinfo dl pthread)
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
