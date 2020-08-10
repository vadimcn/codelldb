add_custom_target(lldb ALL
    COMMAND unzip -o -u ${LLDB_PACKAGE} -d ${CMAKE_CURRENT_BINARY_DIR}
)

# Create dependency check tests
add_test(NAME dependencies:lldb:bin}
    COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/bin ${AllowedDependencies}
)
add_test(NAME dependencies:lldb:lib}
    COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/lib ${AllowedDependencies}
)
