find_package(Python3 COMPONENTS Interpreter Development)

if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(LibPython python3.${Python3_VERSION_MINOR})
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    set(LibPython python3.${Python3_VERSION_MINOR})
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    set(LibPython python3)
endif()

string(JOIN "$<SEMICOLON>" LibPath ${Python3_LIBRARY_DIRS} ${LLDB_ROOT}/lib)

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
    COMMAND ${WithEnv} LIBPATH=\"${LibPath}\"
            cargo build --package=codelldb --bin codelldb --features=weak-linkage ${CargoFlags}
    COMMAND ${CopyFile} ${CargoOutDir}/codelldb${ExeSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_bin"
    USES_TERMINAL
)

add_custom_target(codelldb_lib
    DEPENDS lldb ${PythonFiles}
    COMMAND ${WithEnv} LIBPATH=\"${LibPath}\" LibPython=${LibPython}
            cargo build --package=codelldb --lib --features=weak-linkage ${CargoFlags}
    COMMAND ${CopyFile} ${CargoOutDir}/${DylibPrefix}codelldb${DylibSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_lib"
    USES_TERMINAL
)

set(PATH $ENV{PATH})
string(JOIN ${PathSep} PATH ${PATH} ${LLDB_ROOT}/bin)
string(JOIN ${PathSep} LD_LIBRARY_PATH $ENV{LD_LIBRARY_PATH} ${LLDB_ROOT}/lib)

add_custom_target(cargo_test
    COMMAND ${WithEnv} LIBPATH=\"${LibPath}\" LibPython=${LibPython} PATH=\"${PATH}\" LD_LIBRARY_PATH=\"${LD_LIBRARY_PATH}\" RUST_BACKTRACE=1
            cargo test --target=${LLVM_TRIPLE} -p=lldb -p=debug-protocol -p=codelldb
    USES_TERMINAL
)

# Check that binaries don't depend on liblldb and libpython.
if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    add_test(NAME native:deps:codelldb COMMAND ldd ${CMAKE_CURRENT_BINARY_DIR}/codelldb)
    add_test(NAME native:deps:libcodelldb COMMAND ldd ${CMAKE_CURRENT_BINARY_DIR}/libcodelldb.so)
    set_tests_properties(native:deps:codelldb native:deps:libcodelldb
                         PROPERTIES FAIL_REGULAR_EXPRESSION "libpython;liblldb;libxml;libstdc\\+\\+")
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    add_test(NAME native:deps:codelldb COMMAND otool -L ${CMAKE_CURRENT_BINARY_DIR}/codelldb)
    add_test(NAME native:deps:libcodelldb COMMAND otool -L ${CMAKE_CURRENT_BINARY_DIR}/libcodelldb.dylib)
    set_tests_properties(native:deps:codelldb native:deps:libcodelldb
                         PROPERTIES FAIL_REGULAR_EXPRESSION "libpython;Python;liblldb;libxml")
endif()
