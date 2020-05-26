find_package(Python3 COMPONENTS Interpreter Development)

if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(LLDB_LinkDylib "lldb;python3.${Python3_VERSION_MINOR}m" CACHE STRING "" FORCE)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    set(LLDB_LinkDylib "lldb;python3.${Python3_VERSION_MINOR}" CACHE STRING "" FORCE)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    set(LLDB_LinkDylib "liblldb" CACHE STRING "" FORCE)
else()
    message(FATAL_ERROR "Unsupported platform")
endif()

get_filename_component(LibPythonDir ${Python3_LIBRARY_RELEASE} DIRECTORY)
set(LLDB_LinkSearch "${LLDB_ROOT}/lib;${LibPythonDir}" CACHE STRING "" FORCE)

set(CargoFlags ${CargoFlags} --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml
                             --target=${LLVM_TRIPLE} --target-dir=${CMAKE_BINARY_DIR}/target
                             -Zpackage-features)
if (CMAKE_BUILD_TYPE MATCHES Release|RelWithDebInfo)
    set(CargoFlags ${CargoFlags} --release)
    set(CargoOutDir ${CMAKE_BINARY_DIR}/target/${LLVM_TRIPLE}/release)
else()
    set(CargoOutDir ${CMAKE_BINARY_DIR}/target/${LLVM_TRIPLE}/debug)
endif()

if (CMAKE_VERBOSE_MAKEFILE)
    set(CargoFlags ${CargoFlags} -vv)
endif()

add_copy_files_to(PythonFiles ${CMAKE_CURRENT_BINARY_DIR}
        ${CMAKE_SOURCE_DIR}/adapter2/codelldb.py
        ${CMAKE_SOURCE_DIR}/adapter2/value.py
        ${CMAKE_SOURCE_DIR}/adapter2/debugger.py
        ${CMAKE_SOURCE_DIR}/formatters/rust.py
)

add_custom_target(codelldb ALL
    DEPENDS codelldb_bin codelldb_lib
)

add_custom_target(codelldb_bin
    COMMAND cargo build --package=codelldb --bin codelldb --features=weak-linkage ${CargoFlags}
    COMMAND ${UpdateFile} ${CargoOutDir}/codelldb${ExeSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_bin"
    USES_TERMINAL
)

add_custom_target(codelldb_lib
    DEPENDS lldb ${PythonFiles}
    COMMAND cargo build --package=codelldb --lib --features=weak-linkage ${CargoFlags}
    COMMAND ${UpdateFile} ${CargoOutDir}/${DylibPrefix}codelldb${DylibSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_lib"
    USES_TERMINAL
)

add_custom_target(codelldb_syms
    DEPENDS codelldb_bin codelldb_lib
    COMMAND ${UpdateFile} ${CargoOutDir}/deps/${DylibPrefix}codelldb${DylibSuffix}${SymSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMAND ${UpdateFile} ${CargoOutDir}/codelldb${ExeSuffix}${SymSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Updating codelldb symbols"
    USES_TERMINAL
)

set(PATH $ENV{PATH})
get_filename_component(PythonRuntimeDir "${Python3_RUNTIME_LIBRARY_RELEASE}" DIRECTORY)
string(JOIN ${PathSep} PATH ${PATH} ${LLDB_ROOT}/bin ${PythonRuntimeDir})
string(JOIN ${PathSep} LD_LIBRARY_PATH $ENV{LD_LIBRARY_PATH} ${LLDB_ROOT}/lib)

add_custom_target(cargo_test
    COMMAND ${WithEnv} PATH=\"${PATH}\" LD_LIBRARY_PATH=\"${LD_LIBRARY_PATH}\" RUST_BACKTRACE=1
            cargo test --lib --target=${LLVM_TRIPLE} -p=lldb -p=debug-protocol -p=codelldb
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    USES_TERMINAL
)

# Check that binaries don't depend on liblldb and libpython.
if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    add_test(NAME adapter:deps:codelldb COMMAND ldd ${CMAKE_CURRENT_BINARY_DIR}/codelldb)
    add_test(NAME adapter:deps:libcodelldb COMMAND ldd ${CMAKE_CURRENT_BINARY_DIR}/libcodelldb.so)
    set_tests_properties(adapter:deps:codelldb adapter:deps:libcodelldb
                         PROPERTIES FAIL_REGULAR_EXPRESSION "libpython;liblldb;libxml;libstdc\\+\\+")
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    add_test(NAME adapter:deps:codelldb COMMAND otool -L ${CMAKE_CURRENT_BINARY_DIR}/codelldb)
    add_test(NAME adapter:deps:libcodelldb COMMAND otool -L ${CMAKE_CURRENT_BINARY_DIR}/libcodelldb.dylib)
    set_tests_properties(adapter:deps:codelldb adapter:deps:libcodelldb
                         PROPERTIES FAIL_REGULAR_EXPRESSION "libpython;Python;liblldb;libxml")
endif()
