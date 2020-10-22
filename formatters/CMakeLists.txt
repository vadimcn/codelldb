add_copy_files_to(Formatters ${CMAKE_CURRENT_BINARY_DIR}
    ${CMAKE_SOURCE_DIR}/formatters/__init__.py
    ${CMAKE_SOURCE_DIR}/formatters/rust.py
)

add_custom_target(formatters DEPENDS ${Formatters})
