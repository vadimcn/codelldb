add_custom_target(lldb ALL
    COMMAND unzip -o -u ${LLDB_PACKAGE} -d ${CMAKE_CURRENT_BINARY_DIR}
)

# Create dependency check tests
foreach (Binary ${CheckBinaries})
    add_test(NAME dependencies:${Binary}
        COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${Binary} ${AllowedDependencies})
endforeach()
