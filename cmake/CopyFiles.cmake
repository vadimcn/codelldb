# add_copy_file(
#    <target>                          ## target name
#    <input>                           ## source file path
#    <output>                          ## destination file path
# )

# add_copy_files_to(
#    <target>                          ## target name
#    <destination_dir>                 ## destination directory path
#    <file0> <file1> ...               ## list of files
# )

function(add_copy_file Target Input Output)
    add_custom_command(
        TARGET ${Target}
        PRE_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
            ${Input} ${Output}
        DEPENDS "${Input}"
    )
endfunction()

function(add_copy_files_to Target DestinationDir)
    foreach(File ${ARGN})
        get_filename_component(FName ${File} NAME)
        add_copy_file(${Target} ${File} ${DestinationDir}/${FName})
    endforeach()
endfunction()
