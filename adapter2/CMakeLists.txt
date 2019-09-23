if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(Target x86_64-unknown-linux-gnu)
    if (${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch64")
        set(Target aarch64-unknown-linux-gnu)
    endif()
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    set(RustFlags -Clink-args='-undefined dynamic_lookup')
    set(Target x86_64-apple-darwin)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    find_package(Python3 COMPONENTS Interpreter Development)
    set(RustFlags -L${LLDB_ROOT}/lib -lliblldb)
    set(PythonLibs -L${Python3_LIBRARY_DIRS})
    set(Target x86_64-pc-windows-msvc)
endif()

set(CargoFlags ${CargoFlags} --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml
                             --target=${Target} --target-dir=${CMAKE_BINARY_DIR}/target
                             -Zpackage-features)
if (CMAKE_BUILD_TYPE MATCHES Release|RelWithDebInfo)
    set(CargoFlags ${CargoFlags} --release)
    set(CargoOutDir ${CMAKE_BINARY_DIR}/target/${Target}/release)
else()
    set(CargoOutDir ${CMAKE_BINARY_DIR}/target/${Target}/debug)
endif()

add_copy_files_to(PythonFiles ${CMAKE_CURRENT_BINARY_DIR}
        ${CMAKE_SOURCE_DIR}/adapter2/codelldb.py
        ${CMAKE_SOURCE_DIR}/adapter2/value.py
        ${CMAKE_SOURCE_DIR}/adapter2/debugger.py
        ${CMAKE_SOURCE_DIR}/formatters/rust.py
)

set(copy_file ${CMAKE_COMMAND} -E copy_if_different)

add_custom_target(codelldb ALL
    DEPENDS codelldb_bin codelldb_lib codelldb_python
)

add_custom_target(codelldb_bin
    COMMAND cargo rustc --package=codelldb --bin codelldb ${CargoFlags}
    COMMAND ${copy_file} ${CargoOutDir}/codelldb${ExeSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_bin"
    USES_TERMINAL
)

add_custom_target(codelldb_lib
    DEPENDS lldb
    COMMAND cargo rustc --package=codelldb --lib ${CargoFlags} --no-default-features -- ${RustFlags}
    COMMAND ${copy_file} ${CargoOutDir}/${DylibPrefix}codelldb${DylibSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_lib"
    USES_TERMINAL
)

add_custom_target(codelldb_python
    DEPENDS lldb ${PythonFiles}
    COMMAND cargo rustc --package=codelldb_python --example codelldb_python ${CargoFlags} -- ${RustFlags} ${PythonLibs}
    COMMAND ${copy_file} ${CargoOutDir}/examples/${DylibPrefix}codelldb_python${DylibSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_python"
    USES_TERMINAL
)

add_custom_target(cargo_test
    COMMAND ${CMAKE_COMMAND} -E env RUST_BACKTRACE=1 RUSTFLAGS="-lpython2.7" cargo test ${Features} -p=lldb -p=debug-protocol -p=codelldb
    USES_TERMINAL
)
