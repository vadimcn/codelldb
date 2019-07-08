if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(CPythonFeature python2)
    set(Target x86_64-unknown-linux-gnu)
    if (${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch64")
        set(Target aarch64-unknown-linux-gnu)
    endif()
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    set(CPythonFeature python2)
    # We want to play games with loading liblldb dynamically,
    # so we need to prevent OSX loader from trying to load it on its own.
    set(RustFlags -Clink-args='-undefined dynamic_lookup')
    set(Target x86_64-apple-darwin)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    set(CPythonFeature python3)
    set(RustFlags -L${LLDB_ROOT}/lib -lliblldb)
    set(Target x86_64-pc-windows-msvc)
endif()

set(CargoFlags ${CargoFlags} --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml)
set(CargoFlags ${CargoFlags} -Zpackage-features --no-default-features --features=${CPythonFeature})
set(CargoFlags ${CargoFlags} --target=${Target} --target-dir=${CMAKE_BINARY_DIR}/target)
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

add_custom_target(codelldb ALL
    DEPENDS lldb ${PythonFiles}
    COMMAND cargo rustc --package=codelldb --lib ${CargoFlags} -- ${RustFlags}
    COMMAND cargo rustc --package=codelldb --bin codelldb ${CargoFlags}
    COMMAND ${CMAKE_COMMAND} -E copy_if_different ${CargoOutDir}/codelldb${ExeSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMAND ${CMAKE_COMMAND} -E copy_if_different ${CargoOutDir}/${DylibPrefix}codelldb${DylibSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Running cargo build"
    USES_TERMINAL
)

add_custom_target(cargo_test
    COMMAND ${CMAKE_COMMAND} -E env RUST_BACKTRACE=1 cargo test ${Features} -p=lldb -p=debug-protocol -p=codelldb
    USES_TERMINAL
)
