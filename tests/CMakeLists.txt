
# Tests that can be run under mocha
set(AdapterTestsSrc
    adapter.test.ts
    util.test.ts
    testUtils.ts
)
add_custom_command(
    OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/adapter_tests.js
    DEPENDS ${ExtensionFiles} ${AdapterTestsSrc} tsconfig.json
    COMMAND ${NPM} run webpack -- --mode=${WebpackMode} --stats=minimal
                                  --resolve-modules=${CMAKE_BINARY_DIR}/generated
                                  --output-path=${CMAKE_CURRENT_BINARY_DIR} --output-filename=adapter_tests.js
                                  ${CMAKE_CURRENT_SOURCE_DIR}/adapter.test.ts
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)
add_custom_target(adapter_tests
    DEPENDS package_json ${CMAKE_CURRENT_BINARY_DIR}/adapter_tests.js
)

# Tests that can must be run under vscode-test
set(ExtensionTestsSrc
    extension.test.ts
)
add_custom_command(
    OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/extension_tests.js
    DEPENDS ${ExtensionFiles} ${ExtensionTestsSrc} tsconfig.json
    COMMAND ${NPM} run webpack -- --mode=${WebpackMode} --stats=minimal
                                  --resolve-modules=${CMAKE_BINARY_DIR}/generated
                                  --output-path=${CMAKE_CURRENT_BINARY_DIR} --output-filename=extension_tests.js
                                  ${CMAKE_CURRENT_SOURCE_DIR}/extension.test.ts
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    USES_TERMINAL
)
add_custom_target(extension_tests
    DEPENDS package_json ${CMAKE_CURRENT_BINARY_DIR}/extension_tests.js
)
