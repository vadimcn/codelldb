# add_copy_file(
#    <list>               ## list name to add output path to
#    <input>              ## source file path
#    <output>             ## destination file path
# )

# add_copy_files_to(
#    <list>               ## list name to add output paths to
#    <destination_dir>    ## destination directory path
#    <file0> <file1> ...  ## list of files
# )

function(add_copy_file List Input Output)
    if (IS_SYMLINK ${Input})
        get_filename_component(OutputDir ${Output} DIRECTORY)

        get_filename_component(InputDir ${Input} DIRECTORY)
        get_filename_component(InputDir ${InputDir} REALPATH)
        get_filename_component(SymlinkTgt ${Input} REALPATH)
        file(RELATIVE_PATH SymLinkTgtRel ${InputDir} ${SymlinkTgt})

        add_custom_command(
            OUTPUT ${Output}
            PRE_BUILD
            COMMAND ${CMAKE_COMMAND} -E make_directory ${OutputDir}
            COMMAND ${CMAKE_COMMAND} -E create_symlink ${SymLinkTgtRel} ${Output}
            DEPENDS "${Input}"
            COMMENT "Creating symlink ${Output} -> ${SymLinkTgtRel} "
        )
    else()
        add_custom_command(
            OUTPUT ${Output}
            PRE_BUILD
            COMMAND ${CMAKE_COMMAND} -E copy_if_different ${Input} ${Output}
            DEPENDS "${Input}"
            COMMENT "Copying ${Input} to ${Output}"
        )
    endif()
    list(APPEND ${List} ${Output})
    set(${List} ${${List}} PARENT_SCOPE)
endfunction()

function(add_copy_files_to List DestinationDir)
    foreach(File ${ARGN})
        get_filename_component(FName ${File} NAME)
        add_copy_file(${List} ${File} ${DestinationDir}/${FName})
    endforeach()
    set(${List} ${${List}} PARENT_SCOPE)
endfunction()
