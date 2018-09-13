
if (CMAKE_BUILD_TYPE MATCHES Release|RelWithDebInfo )
    set(CargoFlags --release)
endif()

add_custom_target(codelldb ALL
    DEPENDS lldb
    COMMAND ${CMAKE_COMMAND} -E env LLDB_ROOT=${LLDB_ROOT} cargo build ${CargoFlags} -Zunstable-options --out-dir=${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Running cargo build"
)

add_copy_files(codelldb
    DESTINATION ${CMAKE_CURRENT_BINARY_DIR}
    FILES ${CMAKE_SOURCE_DIR}/adapter2/codelldb.py
          ${CMAKE_SOURCE_DIR}/adapter2/value.py
          ${CMAKE_SOURCE_DIR}/adapter2/debugger.py
          ${CMAKE_SOURCE_DIR}/formatters/rust.py
)
