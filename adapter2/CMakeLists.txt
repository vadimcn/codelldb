
if (CMAKE_BUILD_TYPE MATCHES Release|RelWithDebInfo )
    set(Profile --release)
    set(CargoOutDir ${CMAKE_BINARY_DIR}/target/release)
else()
    set(CargoOutDir ${CMAKE_BINARY_DIR}/target/debug)
endif()

if (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
    set(Features -Zpackage-features --no-default-features --features=python3)
    set(RustFlags -L${LLDB_ROOT}/lib -lliblldb)
    set(ExeSuffix .exe)
    set(DylibSuffix .dll)
else()
    set(Features -Zpackage-features --no-default-features --features=python2)
    set(DylibPrefix lib)

    if (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Darwin")
        # We want to play games with loading liblldb dynamically,
        # so we need to prevent OSX loader from trying to load it on its own.
        set(RustFlags -Clink-args='-undefined dynamic_lookup')
        set(DylibSuffix .dylib)
    else()
        set(DylibSuffix .so)
    endif()
endif()

add_custom_target(codelldb ALL
    DEPENDS lldb
    COMMAND cargo rustc --package=codelldb --lib ${Profile} ${Features} -- ${RustFlags}
    COMMAND cargo rustc --package=codelldb --bin codelldb ${Profile} ${Features}
    COMMENT "Running cargo build"
    USES_TERMINAL
)

add_copy_files(codelldb
    DESTINATION ${CMAKE_CURRENT_BINARY_DIR}
    FILES
        ${CargoOutDir}/codelldb${ExeSuffix}
        ${CargoOutDir}/${DylibPrefix}codelldb${DylibSuffix}
        ${CMAKE_SOURCE_DIR}/adapter2/codelldb.py
        ${CMAKE_SOURCE_DIR}/adapter2/value.py
        ${CMAKE_SOURCE_DIR}/adapter2/debugger.py
        ${CMAKE_SOURCE_DIR}/formatters/rust.py
)

add_custom_target(check_codelldb
    COMMAND cargo check ${Features} --package=codelldb
)

add_custom_target(test_codelldb
    COMMAND ${CMAKE_COMMAND} -E env RUST_BACKTRACE=1 cargo test ${Features} --package=codelldb
)

add_custom_target(doc_lldb
    COMMAND ${CMAKE_COMMAND} -E env LLDB_ROOT=${LLDB_ROOT} cargo doc --package=lldb --no-deps
)
