add_copy_files_to(PythonFiles ${CMAKE_CURRENT_BINARY_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/debugger.py
    ${CMAKE_CURRENT_SOURCE_DIR}/console.py
)

add_copy_files_to(PythonFiles ${CMAKE_CURRENT_BINARY_DIR}/codelldb
    ${CMAKE_CURRENT_SOURCE_DIR}/codelldb/__init__.py
    ${CMAKE_CURRENT_SOURCE_DIR}/codelldb/api.py
    ${CMAKE_CURRENT_SOURCE_DIR}/codelldb/interface.py
    ${CMAKE_CURRENT_SOURCE_DIR}/codelldb/event.py
    ${CMAKE_CURRENT_SOURCE_DIR}/codelldb/value.py
    ${CMAKE_CURRENT_SOURCE_DIR}/codelldb/webview.py
)

add_copy_files_to(PythonFiles ${CMAKE_CURRENT_BINARY_DIR}/codelldb/commands
    ${CMAKE_CURRENT_SOURCE_DIR}/codelldb/commands/__init__.py
    ${CMAKE_CURRENT_SOURCE_DIR}/codelldb/commands/debug_info.py
    ${CMAKE_CURRENT_SOURCE_DIR}/codelldb/commands/nofail.py
)

add_custom_target(codelldb_scripts
    DEPENDS ${PythonFiles}
)
