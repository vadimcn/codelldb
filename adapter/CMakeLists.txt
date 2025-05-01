include(ExternalProject)

add_subdirectory(scripts)

if (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    find_file(MSDIA msdia140.dll PATHS "${CMAKE_SOURCE_DIR}" "$ENV{VSINSTALLDIR}/DIA SDK/bin/amd64" NO_DEFAULT_PATH)
    if (NOT MSDIA) # Search again with default paths
        find_file(MSDIA msdia140.dll)
    endif()

    if (NOT MSDIA)
        message(FATAL_ERROR "msdia140.dll not found")
    else()
        execute_process(COMMAND dumpbin "/headers" "${MSDIA}" OUTPUT_VARIABLE OUTPUT)
        if (OUTPUT MATCHES ".*machine \\(x64\\).*")
            message ("Found MSDIA at ${MSDIA}")
            add_copy_file(DIAFiles "${MSDIA}" ${CMAKE_CURRENT_BINARY_DIR}/msdia140.dll)
        else()
            message(FATAL_ERROR "Found MSDIA at ${MSDIA}, but it isn't an x64 binary.")
        endif()
    endif()
endif()

if (CMAKE_VERBOSE_MAKEFILE)
    set(CargoFlags ${CargoFlags} -vv)
endif()

add_custom_target(adapter
    DEPENDS codelldb_exe codelldb_scripts ${DIAFiles}
)

set(CodelldbExe codelldb${ExeSuffix})
set(CargoOutDir ${CMAKE_BINARY_DIR}/target/${LLVM_TRIPLE}/${CargoBuildType})

if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(RustcFlags
        -Csplit-debuginfo=packed
        -Clink-arg=-fuse-ld=lld
        -Clink-arg=-Wl,-Bstatic,-lc++,-lc++abi,-Bdynamic
    )
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    set(RustcFlags
        -Csplit-debuginfo=packed
        -Clink-arg=-lc++
    )
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")

endif()

externalproject_add(cargo_build
    DOWNLOAD_COMMAND ""
    CONFIGURE_COMMAND ""
    SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}"
    BINARY_DIR "${CMAKE_CURRENT_BINARY_DIR}"
    BUILD_COMMAND cargo rustc --package=codelldb --bin=codelldb ${CargoFlags} --features=no_link_args -- ${RustcFlags}
    BUILD_BYPRODUCTS ${CargoOutDir}/${CodelldbExe}
    BUILD_ALWAYS ON
    USES_TERMINAL_BUILD ON
    INSTALL_COMMAND ""
)

add_dependencies(cargo_build lldb)

# Exe
add_custom_target(codelldb_exe
    COMMAND ${UpdateFile} ${CargoOutDir}/${CodelldbExe} ${CMAKE_CURRENT_BINARY_DIR}
    DEPENDS cargo_build
    USES_TERMINAL
)

add_test(NAME cargo_test
    COMMAND cargo test --lib -p=lldb -p=adapter-protocol -p=codelldb ${CargoFlags} -vv -- --test-threads=1
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)

# Create dependency check tests
add_test(NAME dependencies:adapter
    COMMAND ${Python} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR} ${AllowedDependencies}
)
