if (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    find_file(MSDIA msdia140.dll PATHS "$ENV{VSINSTALLDIR}/DIA SDK/bin/amd64")
    if (NOT MSDIA)
        message(WARNING "msdiaXXX.dll not found")
    else()
        message ("Found MSDIA at ${MSDIA}")
        add_copy_file(LLDBFiles "${MSDIA}" ${CMAKE_CURRENT_BINARY_DIR}/bin/msdia140.dll)
    endif()
endif()

add_custom_target(lldb ALL DEPENDS ${LLDBFiles}
    COMMAND unzip -o -u ${LLDB_PACKAGE} -d ${CMAKE_CURRENT_BINARY_DIR}
)

# Create dependency check tests
add_test(NAME dependencies:lldb:bin}
    COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/bin ${AllowedDependencies}
)
add_test(NAME dependencies:lldb:lib}
    COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/lib ${AllowedDependencies}
)
