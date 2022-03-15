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

if (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    set(LLDB_LinkDylib "liblldb" CACHE STRING "" FORCE)
else()
    set(LLDB_LinkDylib "lldb" CACHE STRING "" FORCE)
endif()
set(LLDB_LinkSearch "${CMAKE_BINARY_DIR}/lldb/lib" CACHE STRING "" FORCE)

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

set(Cargo ${WithEnv} CXX_${LLVM_TRIPLE}=${CMAKE_C_COMPILER} CXXFLAGS_${LLVM_TRIPLE}=${CMAKE_C_FLAGS} cargo)

add_copy_files_to(PythonFiles ${CMAKE_CURRENT_BINARY_DIR}
        ${CMAKE_SOURCE_DIR}/adapter/codelldb.py
        ${CMAKE_SOURCE_DIR}/adapter/value.py
        ${CMAKE_SOURCE_DIR}/adapter/debugger.py
        ${CMAKE_SOURCE_DIR}/adapter/console.py
)

add_custom_target(codelldb ALL
    DEPENDS codelldb_bin codelldb_lib ${DIAFiles}
)

add_custom_target(codelldb_bin
    COMMAND ${Cargo} build --package=codelldb --bin codelldb --features=weak-linkage ${CargoFlags}
    COMMAND ${UpdateFile} ${CargoOutDir}/codelldb${ExeSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_bin"
    USES_TERMINAL
)

add_custom_target(codelldb_lib
    DEPENDS ${PythonFiles}
    COMMAND ${Cargo} build --package=codelldb --lib --features=weak-linkage ${CargoFlags}
    COMMAND ${UpdateFile} ${CargoOutDir}/${DylibPrefix}codelldb${DylibSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_lib"
    USES_TERMINAL
)

add_custom_target(codelldb_syms
    DEPENDS codelldb_bin codelldb_lib
    COMMAND ${UpdateFile} ${CargoOutDir}/deps/${DylibPrefix}codelldb${DylibSuffix}${SymSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMAND ${UpdateFile} ${CargoOutDir}/codelldb${ExeSuffix}${SymSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Updating codelldb symbols"
    USES_TERMINAL
)

set(PATH $ENV{PATH}) # keep
string(JOIN ${PathSep} PATH ${CMAKE_BINARY_DIR}/lldb/bin ${PATH} )
string(JOIN ${PathSep} LD_LIBRARY_PATH ${LLDB_LinkSearch} $ENV{LD_LIBRARY_PATH})
add_custom_target(cargo_test
    COMMAND echo PATH=${PATH}
    COMMAND echo LD_LIBRARY_PATH=${LD_LIBRARY_PATH}
    COMMAND ${WithEnv} PATH=\"${PATH}\" LD_LIBRARY_PATH=\"${LD_LIBRARY_PATH}\" RUST_BACKTRACE=1
            ${Cargo} test --lib -p=lldb -p=adapter-protocol -p=codelldb ${CargoFlags} -- --test-threads=1
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    USES_TERMINAL
)

# Create dependency check tests
add_test(NAME dependencies:adapter}
    COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tools/check_dependencies.py ${CMAKE_CURRENT_BINARY_DIR} ${AllowedDependencies}
)
