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
    cpp/types.cpp
    cpp/no_line_info.cpp # without debug info, for testing disassembly
    cpp/denorm_path.cpp  # test debugger's path normalization
    cpp/relative_path.cpp  # test source map
    cpp/remote1/remote_path.cpp  # test source map
    cpp/remote2/remote_path.cpp  # test source map
)

if (NOT ${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    target_link_libraries(debuggee PRIVATE dl pthread)
    if ("${CMAKE_CXX_COMPILER_ID}" STREQUAL "Clang")
        target_compile_options(debuggee PRIVATE "-fstandalone-debug") # Include STL debug info in the binary.
    endif()
else()
    set(Drive "C:")
endif()

if ("${LLVM_TRIPLE}" MATCHES "-msvc")
    set_source_files_properties(cpp/no_line_info.cpp PROPERTIES COMPILE_FLAGS /DEBUG:None)
else()
    set_source_files_properties(cpp/no_line_info.cpp PROPERTIES COMPILE_FLAGS -g0)
    set_source_files_properties(cpp/remote1/remote_path.cpp PROPERTIES COMPILE_FLAGS -fdebug-prefix-map=${CMAKE_SOURCE_DIR}/cpp/remote1=${Drive}/remote1)
    set_source_files_properties(cpp/remote2/remote_path.cpp PROPERTIES COMPILE_FLAGS -fdebug-prefix-map=${CMAKE_SOURCE_DIR}/cpp/remote2=${Drive}/remote2)
    set_source_files_properties(cpp/relative_path.cpp PROPERTIES COMPILE_FLAGS "-fdebug-prefix-map=${CMAKE_SOURCE_DIR}/cpp=cpp -fdebug-prefix-map=${CMAKE_BINARY_DIR}=.")
    set_source_files_properties(cpp/denorm_path.cpp PROPERTIES COMPILE_FLAGS -fdebug-prefix-map=${CMAKE_SOURCE_DIR}/cpp=../../debuggee/cpp/../cpp/.)
endif()

# rusttypes
if (NOT ${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    set(Target)
    set(Rusttypes rusttypes)
else()
    set(Target --target ${LLVM_TRIPLE})
    set(Rusttypes rusttypes.exe)
endif()

set(Cargo ${CMAKE_COMMAND} -E env RUSTC_BOOTSTRAP=1 cargo -Zunstable-options)

add_custom_target(rust-debuggee
    COMMAND ${Cargo} build -p rust-debuggee --bin rust-debuggee ${Target} --out-dir ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)

add_dependencies(debuggee debuggee2 rust-debuggee)
