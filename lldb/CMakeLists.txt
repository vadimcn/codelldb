if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb)
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb-server ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb-server)
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb-argdumper ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb-argdumper)
    set(LibLLDBGlob lib/liblldb.so*)
    set(PythonGlob lib/python3*/*-packages/*)
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb)
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/debugserver ${CMAKE_CURRENT_BINARY_DIR}/bin/debugserver)
    add_copy_file(LLDBFiles ${LLDB_ROOT}/bin/lldb-argdumper ${CMAKE_CURRENT_BINARY_DIR}/bin/lldb-argdumper)
    set(LibLLDBGlob lib/liblldb.*dylib)
    set(PythonGlob lib/python3*/*-packages/*)
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
    set(LibLLDBGlob bin/liblldb.dll)
    set(PythonGlob lib/site-packages/*)
else()
    message(FATAL_ERROR "Unsupported platform")
endif()

file(GLOB List
    RELATIVE ${LLDB_ROOT}
    ${LLDB_ROOT}/${LibLLDBGlob})
foreach(File ${List})
    if (NOT IS_SYMLINK ${LLDB_ROOT}/${File})
        add_copy_file(LLDBFiles ${LLDB_ROOT}/${File} ${CMAKE_CURRENT_BINARY_DIR}/${File})
    endif()
endforeach()

file(GLOB_RECURSE PythonFiles
    LIST_DIRECTORIES false
    RELATIVE ${LLDB_ROOT}
    ${LLDB_ROOT}/${PythonGlob})
list(FILTER PythonFiles EXCLUDE REGEX "_lldb.*")
foreach(File ${PythonFiles})
    add_copy_file(LLDBFiles ${LLDB_ROOT}/${File} ${CMAKE_CURRENT_BINARY_DIR}/${File})
endforeach()

add_custom_target(lldb ALL DEPENDS ${LLDBFiles})
