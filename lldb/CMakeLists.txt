
function(add_copy_liblldb Target RelativeTo Pattern)
    file(GLOB List
        RELATIVE ${RelativeTo}
        ${RelativeTo}/${Pattern})

    foreach(File ${List})
        if (NOT IS_SYMLINK ${RelativeTo}/${File})
            add_copy_file(LLDBFiles ${RelativeTo}/${File} ${CMAKE_CURRENT_BINARY_DIR}/${File})
        endif()
    endforeach()
    set(LLDBFiles ${LLDBFiles} PARENT_SCOPE)
endfunction()

function (add_copy_python_files Target RelativeTo Pattern)
    file(GLOB_RECURSE PythonFiles
        LIST_DIRECTORIES false
        RELATIVE ${RelativeTo}
        ${RelativeTo}/${Pattern})
    list(FILTER PythonFiles EXCLUDE REGEX "_lldb.*")
    foreach(File ${PythonFiles})
        add_copy_file(LLDBFiles ${RelativeTo}/${File} ${CMAKE_CURRENT_BINARY_DIR}/${File})
    endforeach()
    set(LLDBFiles ${LLDBFiles} PARENT_SCOPE)
endfunction()

if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb)
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb-server ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb-server)
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb-argdumper ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb-argdumper)
    add_copy_liblldb(LLDBFiles ${LLDB_ROOT} lib/liblldb.so*)
    add_copy_python_files(LLDBFiles ${LLDB_ROOT} lib/python3*/site-packages/*)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb)
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/debugserver ${CMAKE_CURRENT_BINARY_DIR}/bin/debugserver)
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb-argdumper ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb-argdumper)
    add_copy_liblldb(LLDBFiles ${LLDB_ROOT} lib/liblldb.*dylib)
    add_copy_python_files(LLDBFiles ${LLDB_ROOT} lib/python3*/site-packages/*)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    find_file(MSDIA msdia140.dll PATHS "$ENV{VSINSTALLDIR}/DIA SDK/bin/amd64")
    if (NOT MSDIA)
        message(WARNING "msdiaXXX.dll not found")
    else()
        message ("Found MSDIA at ${MSDIA}")
    endif()

    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb.exe ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb.exe)
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb-argdumper.exe ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb-argdumper.exe)
    add_copy_file(LLDBFiles "${MSDIA}" ${CMAKE_CURRENT_BINARY_DIR}/bin/msdia140.dll)
    add_copy_liblldb(LLDBFiles ${LLDB_ROOT} bin/liblldb.dll)
    add_copy_python_files(LLDBFiles ${LLDB_ROOT} lib/site-packages/*)
else()
    message(FATAL_ERROR)
endif()

add_custom_target(lldb ALL DEPENDS ${LLDBFiles})
