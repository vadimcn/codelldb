add_custom_target(lldb ALL DEPENDS ${LLDB_PACKAGE})
if(NOT IS_DIRECTORY ${LLDB_PACKAGE})
    file(REMOVE ${CMAKE_CURRENT_BINARY_DIR}/bin) # Remove symlinks, if any
    file(REMOVE ${CMAKE_CURRENT_BINARY_DIR}/lib)
    add_custom_command(TARGET lldb
        COMMAND unzip -o -u ${LLDB_PACKAGE} -d ${CMAKE_CURRENT_BINARY_DIR}
    )
else()
    message("${LLDB_PACKAGE} is a directory, symlinking.")
    file(REMOVE_RECURSE ${CMAKE_CURRENT_BINARY_DIR}/bin) # Remove dirs, if any
    file(REMOVE_RECURSE ${CMAKE_CURRENT_BINARY_DIR}/lib)
    file(CREATE_LINK ${LLDB_PACKAGE}/bin ${CMAKE_CURRENT_BINARY_DIR}/bin SYMBOLIC)
    file(CREATE_LINK ${LLDB_PACKAGE}/lib ${CMAKE_CURRENT_BINARY_DIR}/lib SYMBOLIC)
endif()


set(LLDBAllowedDependencies "${AllowedDependencies}|liblldb.*|(lib)?python3.*")

# Create dependency check tests
add_test(NAME dependencies:lldb:bin}
    COMMAND ${Python} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/bin ${LLDBAllowedDependencies}
)
add_test(NAME dependencies:lldb:lib}
    COMMAND ${Python} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/lib ${LLDBAllowedDependencies}
)
