
if (CMAKE_BUILD_TYPE MATCHES Release|RelWithDebInfo )
    set(Profile --release)
endif()

if (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
    set(Features -Zpackage-features --no-default-features --features=python3)
else()
    set(Features -Zpackage-features --no-default-features --features=python2)
endif()

add_custom_target(codelldb ALL
    DEPENDS lldb
    COMMAND ${CMAKE_COMMAND} -E env LLDB_ROOT=${LLDB_ROOT} cargo build ${Profile} ${Features} --package=codelldb -Zunstable-options --out-dir=${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Running cargo build"
)

add_copy_files(codelldb
    DESTINATION ${CMAKE_CURRENT_BINARY_DIR}
    FILES ${CMAKE_SOURCE_DIR}/adapter2/codelldb.py
          ${CMAKE_SOURCE_DIR}/adapter2/value.py
          ${CMAKE_SOURCE_DIR}/adapter2/debugger.py
          ${CMAKE_SOURCE_DIR}/formatters/rust.py
)

add_custom_target(check_codelldb
    COMMAND cargo check ${Features} --package=codelldb
)

add_custom_target(test_codelldb
    COMMAND cargo test ${Features} --package=codelldb
)
