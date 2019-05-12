add_custom_target(lldb ALL)

if (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Linux")
    add_copy_file(lldb ${LLDB_ROOT}/bin/lldb ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb)
    add_copy_file(lldb ${LLDB_ROOT}/bin/lldb-server ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb-server)
    add_copy_file(lldb ${LLDB_ROOT}/lib/liblldb.so.8-custom ${CMAKE_CURRENT_BINARY_DIR}/lib/liblldb.so.8-custom)

    file(GLOB_RECURSE PythonFiles
        LIST_DIRECTORIES false
        RELATIVE ${LLDB_ROOT}
        ${LLDB_ROOT}/lib/python2.7/site-packages/*)
    list(FILTER PythonFiles EXCLUDE REGEX "_lldb.*")
    foreach(File ${PythonFiles})
        add_copy_file(lldb ${LLDB_ROOT}/${File} ${CMAKE_CURRENT_BINARY_DIR}/${File})
    endforeach()

elseif (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Darwin")
    add_copy_file(lldb ${LLDB_ROOT}/bin/lldb ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb)
    add_copy_file(lldb ${LLDB_ROOT}/bin/debugserver ${CMAKE_CURRENT_BINARY_DIR}/bin/debugserver)
    add_copy_file(lldb ${LLDB_ROOT}/lib/liblldb.8.0.0-custom.dylib ${CMAKE_CURRENT_BINARY_DIR}/lib/liblldb.8.0.0-custom.dylib)

    file(GLOB_RECURSE PythonFiles
        LIST_DIRECTORIES false
        RELATIVE ${LLDB_ROOT}
        ${LLDB_ROOT}/lib/python2.7/site-packages/*)
    list(FILTER PythonFiles EXCLUDE REGEX "_lldb.*")
    foreach(File ${PythonFiles})
        add_copy_file(lldb ${LLDB_ROOT}/${File} ${CMAKE_CURRENT_BINARY_DIR}/${File})
    endforeach()

elseif (${CMAKE_HOST_SYSTEM_NAME} STREQUAL "Windows")
    find_file(MSDIA msdia140.dll PATHS "$ENV{VSINSTALLDIR}/DIA SDK/bin/amd64")
    if (NOT MSDIA)
        message(WARNING "msdiaXXX.dll not found")
        set(MSDIA)
    else()
        message ("Found MSDIA at ${MSDIA}")
    endif()

    add_copy_file(lldb ${LLDB_ROOT}/bin/lldb.exe ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb.exe)
    add_copy_file(lldb ${LLDB_ROOT}/bin/liblldb.dll ${CMAKE_CURRENT_BINARY_DIR}/bin/liblldb.dll)
    add_copy_file(lldb "${MSDIA}" ${CMAKE_CURRENT_BINARY_DIR}/bin/msdia140.dll)

    file(GLOB_RECURSE PythonFiles
        LIST_DIRECTORIES false
        RELATIVE ${LLDB_ROOT}
        ${LLDB_ROOT}/lib/site-packages/*)
    list(FILTER PythonFiles EXCLUDE REGEX "_lldb.*")
    foreach(File ${PythonFiles})
        add_copy_file(lldb ${LLDB_ROOT}/${File} ${CMAKE_CURRENT_BINARY_DIR}/${File})
    endforeach()

else()
    message(FATAL_ERROR "${CMAKE_HOST_SYSTEM_NAME} not supported." )
endif()
