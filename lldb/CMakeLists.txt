add_custom_target(lldb ALL DEPENDS ${LLDB_PACKAGE})
if (NOT IS_DIRECTORY ${LLDB_PACKAGE})
    file(REMOVE ${CMAKE_CURRENT_BINARY_DIR}/bin) # Remove symlinks, if any
    file(REMOVE ${CMAKE_CURRENT_BINARY_DIR}/lib)
    add_custom_command(TARGET lldb POST_BUILD
        COMMAND ${CMAKE_COMMAND} -DINPUT=${LLDB_PACKAGE} -DDESTINATION=${CMAKE_CURRENT_BINARY_DIR} -P "${CMAKE_SOURCE_DIR}/cmake/Extract.cmake"
    )
    set(LLDB_INCLUDE "${CMAKE_CURRENT_BINARY_DIR}/include" PARENT_SCOPE)

    # Create dependency check tests
    set(LLDBAllowedDependencies "${AllowedDependencies}|liblldb.*|(lib)?python3.*")
    add_test(NAME dependencies:lldb:bin
        COMMAND ${Python} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/bin ${LLDBAllowedDependencies}
    )
    add_test(NAME dependencies:lldb:lib
        COMMAND ${Python} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR}/lib ${LLDBAllowedDependencies}
    )
else()
    message("${LLDB_PACKAGE} is a directory, symlinking.")
    file(REMOVE_RECURSE ${CMAKE_CURRENT_BINARY_DIR}/bin) # Remove dirs, if any
    file(REMOVE_RECURSE ${CMAKE_CURRENT_BINARY_DIR}/lib)
    file(CREATE_LINK ${LLDB_PACKAGE}/bin ${CMAKE_CURRENT_BINARY_DIR}/bin SYMBOLIC)
    file(CREATE_LINK ${LLDB_PACKAGE}/lib ${CMAKE_CURRENT_BINARY_DIR}/lib SYMBOLIC)
    if (NOT DEFINED LLVM_SOURCE_DIR)
        set(LLVM_SOURCE_DIR "${LLDB_PACKAGE}/..")
    endif()
    set(LLDB_INCLUDE "${LLVM_SOURCE_DIR}/lldb/include;${LLDB_PACKAGE}/tools/lldb/include" PARENT_SCOPE)

    # Don't check dependencies in symlinked LLDB, as it contains many files that won't be included in the package.
endif()
