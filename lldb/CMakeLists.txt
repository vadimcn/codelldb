add_custom_target(lldb ALL DEPENDS ${LLDB_PACKAGE})
if(NOT IS_DIRECTORY ${LLDB_PACKAGE})
    file(REMOVE ${CMAKE_CURRENT_BINARY_DIR}/bin) # Remove symlinks, if any
    file(REMOVE ${CMAKE_CURRENT_BINARY_DIR}/lib)
    add_custom_command(TARGET lldb
        COMMAND unzip -o -u ${LLDB_PACKAGE} -d ${CMAKE_CURRENT_BINARY_DIR}
    )
else()
    file(REMOVE_RECURSE ${CMAKE_CURRENT_BINARY_DIR}/bin) # Remove dirs, if any
    file(REMOVE_RECURSE ${CMAKE_CURRENT_BINARY_DIR}/lib)
    if(WIN32)
        # Symbolic link does not work when installing the locally build vsix
        message("${LLDB_PACKAGE} is a directory, copying.")
        file(COPY ${LLDB_PACKAGE}/bin DESTINATION ${CMAKE_CURRENT_BINARY_DIR})
        file(COPY ${LLDB_PACKAGE}/lib DESTINATION ${CMAKE_CURRENT_BINARY_DIR})
    else()
        message("${LLDB_PACKAGE} is a directory, symlinking.")
        file(CREATE_LINK ${LLDB_PACKAGE}/bin ${CMAKE_CURRENT_BINARY_DIR}/bin SYMBOLIC)
        file(CREATE_LINK ${LLDB_PACKAGE}/lib ${CMAKE_CURRENT_BINARY_DIR}/lib SYMBOLIC)
    endif()
endif()

# Create dependency check tests
add_test(NAME dependencies:lldb:bin}
    COMMAND ${Python} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/bin ${AllowedDependencies}
)
add_test(NAME dependencies:lldb:lib}
    COMMAND ${Python} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/lib ${AllowedDependencies}
)
