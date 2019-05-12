function(add_typescript Target)
    file(GLOB Sources
        RELATIVE ${CMAKE_CURRENT_SOURCE_DIR}
        ${CMAKE_CURRENT_SOURCE_DIR}/*.ts)

    foreach(File ${Sources})
        get_filename_component(FileName ${File} NAME)
        string(REPLACE ".ts" ".js" FileName ${FileName})
        list(APPEND Outputs ${CMAKE_CURRENT_BINARY_DIR}/${FileName})
    endforeach()

    add_custom_target(${Target}
        DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/tsconfig.json ${Outputs}
    )
    add_custom_command(
        OUTPUT ${Outputs}
        DEPENDS ${Sources} ${CMAKE_CURRENT_SOURCE_DIR}/tsconfig.json
        COMMAND ${CMAKE_COMMAND} -E env "${CMAKE_BINARY_DIR}/node_modules/.bin/tsc${NodeProgExt}" --project ${CMAKE_CURRENT_SOURCE_DIR} --outDir ${CMAKE_CURRENT_BINARY_DIR} #--traceResolution
        COMMENT "Building ${Target}"
    )
    set_directory_properties(PROPERTIES ADDITIONAL_MAKE_CLEAN_FILES "${Outputs}")
endfunction()
