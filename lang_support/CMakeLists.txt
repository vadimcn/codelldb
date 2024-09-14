add_copy_files_to(Languages ${CMAKE_CURRENT_BINARY_DIR}
    ${CMAKE_SOURCE_DIR}/lang_support/__init__.py
    ${CMAKE_SOURCE_DIR}/lang_support/rust.py
)

add_custom_target(lang_support DEPENDS ${Languages})
