file(GLOB_RECURSE ExtensionFiles ${CMAKE_CURRENT_SOURCE_DIR}/*.ts ${CMAKE_CURRENT_SOURCE_DIR}/*.json)
add_custom_command(
    OUTPUT extension.js
    DEPENDS ${ExtensionFiles}
    COMMAND ${NPM} run webpack -- --mode=${WebpackMode} --stats=minimal
                                  --resolve-modules=${CMAKE_BINARY_DIR}/generated
                                  --output-path=${CMAKE_BINARY_DIR} --output-filename=extension.js
                                  ${CMAKE_CURRENT_SOURCE_DIR}/main.ts
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)
add_custom_target(extension
    DEPENDS package_json extension.js
)
