if (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    set(LLDB_LinkDylib "liblldb" CACHE STRING "" FORCE)
else()
    set(LLDB_LinkDylib "lldb" CACHE STRING "" FORCE)
endif()
set(LLDB_LinkSearch "${CMAKE_BINARY_DIR}/lldb/lib" CACHE STRING "" FORCE)

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
        ${CMAKE_SOURCE_DIR}/adapter/codelldb.py
        ${CMAKE_SOURCE_DIR}/adapter/value.py
        ${CMAKE_SOURCE_DIR}/adapter/debugger.py
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

string(JOIN ${PathSep} PATH $ENV{PATH} ${LLDB_ROOT}/bin)
string(JOIN ${PathSep} LD_LIBRARY_PATH $ENV{LD_LIBRARY_PATH} ${LLDB_LinkSearch})
add_custom_target(cargo_test
    COMMAND ${WithEnv} PATH=\"${PATH}\" LD_LIBRARY_PATH=\"${LD_LIBRARY_PATH}\" RUST_BACKTRACE=1
            cargo test --lib -p=lldb -p=debug-protocol -p=codelldb ${CargoFlags}
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    USES_TERMINAL
)

# Create dependency check tests
foreach (Binary codelldb${ExeSuffix} ${DylibPrefix}codelldb${DylibSuffix})
    add_test(NAME dependencies:${Binary}
        COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/${Binary} ${AllowedDependencies})
endforeach()
