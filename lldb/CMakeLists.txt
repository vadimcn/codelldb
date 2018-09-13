add_custom_target(lldb ALL)

if (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Linux")
    add_copy_files(lldb
        DESTINATION ${CMAKE_CURRENT_BINARY_DIR}/bin
        FILES ${LLDB_ROOT}/bin/lldb ${LLDB_ROOT}/bin/lldb-server
    )
    add_copy_files(lldb
        DESTINATION ${CMAKE_CURRENT_BINARY_DIR}/lib
        FILES ${LLDB_ROOT}/lib/liblldb.so.8-custom
    )
    add_copy_directory(lldb
        ${LLDB_ROOT}/lib/python2.7/site-packages
        DESTINATION ${CMAKE_CURRENT_BINARY_DIR}
        RELATIVE ${LLDB_ROOT}
        EXCLUDE "_lldb.*"
    )
elseif (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Darwin")
    add_copy_files(lldb
        DESTINATION ${CMAKE_CURRENT_BINARY_DIR}/bin
        FILES ${LLDB_ROOT}/bin/lldb ${LLDB_ROOT}/bin/debugserver
    )
    add_copy_files(lldb
        DESTINATION ${CMAKE_CURRENT_BINARY_DIR}/lib
        FILES ${LLDB_ROOT}/lib/liblldb.8.0.0.dylib
    )
    add_copy_directory(lldb
        ${LLDB_ROOT}/lib/python2.7/site-packages
        DESTINATION ${CMAKE_CURRENT_BINARY_DIR}
        RELATIVE ${LLDB_ROOT}
        EXCLUDE "_lldb.*"
    )
elseif (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
    find_file(MSDIA msdia140.dll PATHS "$ENV{VSINSTALLDIR}/DIA SDK/bin/amd64")
    if (NOT MSDIA)
        message(WARNING "msdiaXXX.dll not found")
        set(MSDIA)
    else()
        message ("Found MSDIA at ${MSDIA}")
    endif()

    add_copy_files(lldb
        DESTINATION ${CMAKE_CURRENT_BINARY_DIR}/bin
        FILES ${LLDB_ROOT}/bin/lldb.exe
    )
    add_copy_files(lldb
        DESTINATION ${CMAKE_CURRENT_BINARY_DIR}/bin
        FILES ${LLDB_ROOT}/bin/liblldb.dll "${MSDIA}"
    )
    add_copy_directory(lldb
        ${LLDB_ROOT}/lib/site-packages
        DESTINATION ${CMAKE_CURRENT_BINARY_DIR}
        RELATIVE ${LLDB_ROOT}
        EXCLUDE "_lldb.*"
    )
else()
    message(FATAL_ERROR "${CMAKE_HOST_SYSTEM_NAME} not supported." )
endif()

