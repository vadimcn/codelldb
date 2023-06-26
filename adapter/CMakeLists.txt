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

set(CargoFlags ${CargoFlags} --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml
                             --target=${LLVM_TRIPLE} --target-dir=${CMAKE_BINARY_DIR}/target)
if (CMAKE_BUILD_TYPE MATCHES Release|RelWithDebInfo)
    set(CargoFlags ${CargoFlags} --release)
    set(CargoOutDir ${CMAKE_BINARY_DIR}/target/${LLVM_TRIPLE}/release)
else()
    set(CargoOutDir ${CMAKE_BINARY_DIR}/target/${LLVM_TRIPLE}/debug)
endif()

if (CMAKE_VERBOSE_MAKEFILE)
    set(CargoFlags ${CargoFlags} -vv)
endif()

add_custom_target(adapter
    DEPENDS codelldb_exe codelldb_dylib codelldb_scripts ${DIAFiles}
)

set(CodelldbExe codelldb${ExeSuffix})
set(CodelldbLib ${LibPrefix}codelldb${LibSuffix})

externalproject_add(cargo_build
    DOWNLOAD_COMMAND ""
    CONFIGURE_COMMAND ""
    SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}"
    BINARY_DIR "${CMAKE_CURRENT_BINARY_DIR}"
    BUILD_COMMAND cargo build --package=codelldb --bin codelldb --lib --features=weak-linkage ${CargoFlags}
    BUILD_BYPRODUCTS ${CargoOutDir}/${CodelldbExe} ${CargoOutDir}/${CodelldbLib}
    BUILD_ALWAYS ON
    USES_TERMINAL_BUILD ON
    INSTALL_COMMAND ""
)

# Exe
add_custom_target(codelldb_exe
    COMMAND ${UpdateFile} ${CargoOutDir}/${CodelldbExe} ${CMAKE_CURRENT_BINARY_DIR}
    DEPENDS cargo_build
    USES_TERMINAL
)

# Dylib
if (NOT ${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/empty.c "")
    add_library(codelldb_dylib SHARED ${CMAKE_CURRENT_BINARY_DIR}/empty.c)
    if (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
        target_link_options(codelldb_dylib PRIVATE --force-link=_entry
                            -lc++ -lm -lc -undefined dynamic_lookup)
    elseif(${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
        target_link_options(codelldb_dylib PRIVATE --force-link=entry
                            -static-libgcc -Wl,-Bstatic -lc++ -lc++abi -Wl,-Bdynamic -ldl -lm -lc)
    elseif(${CMAKE_SYSTEM_NAME} STREQUAL "FreeBSD")
        target_link_options(codelldb_dylib PRIVATE -Wl,--undefined=entry -ldl -lm -lc)
    else()
        message(FATAL_ERROR "Unsupported platform ${CMAKE_SYSTEM_NAME}")
    endif()
else()
    file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/empty.c "")
    file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/exports.def "EXPORTS\r\n  entry")
    add_library(codelldb_dylib SHARED ${CMAKE_CURRENT_BINARY_DIR}/empty.c ${CMAKE_CURRENT_BINARY_DIR}/exports.def)
    target_link_directories(codelldb_dylib PRIVATE ${CMAKE_BINARY_DIR}/lldb/lib)
    target_link_libraries(codelldb_dylib PRIVATE liblldb.lib ws2_32.lib bcrypt.lib userenv.lib ntdll.lib)
    target_link_options(codelldb_dylib PRIVATE /NODEFAULTLIB:MSVCRTD)
endif()
add_dependencies(codelldb_dylib cargo_build)
set_target_properties(codelldb_dylib PROPERTIES OUTPUT_NAME "codelldb")
target_link_libraries(codelldb_dylib PRIVATE ${CargoOutDir}/${CodelldbLib})

add_custom_target(cargo_test
    COMMAND cargo test --lib -p=lldb -p=adapter-protocol -p=codelldb ${CargoFlags} -- --test-threads=1
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    USES_TERMINAL
)

# Create dependency check tests
add_test(NAME dependencies:adapter}
    COMMAND ${Python} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR} ${AllowedDependencies}
)
