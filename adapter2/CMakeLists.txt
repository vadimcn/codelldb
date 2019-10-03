find_package(Python3 COMPONENTS Interpreter Development)

if (${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(Target x86_64-unknown-linux-gnu)
    if (${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch64")
        set(Target aarch64-unknown-linux-gnu)
    endif()
    set(LibPython python3.${Python3_VERSION_MINOR})
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Darwin")
    set(Target x86_64-apple-darwin)
    set(LibPython python3.${Python3_VERSION_MINOR})
elseif (${CMAKE_SYSTEM_NAME} STREQUAL "Windows")
    set(Target x86_64-pc-windows-msvc)
    set(LibPython python3)
endif()

string(JOIN "$<SEMICOLON>" LibPath ${Python3_LIBRARY_DIRS} ${LLDB_ROOT}/lib)

set(CargoFlags ${CargoFlags} --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml
                             --target=${Target} --target-dir=${CMAKE_BINARY_DIR}/target
                             -Zpackage-features)
if (CMAKE_BUILD_TYPE MATCHES Release|RelWithDebInfo)
    set(CargoFlags ${CargoFlags} --release)
    set(CargoOutDir ${CMAKE_BINARY_DIR}/target/${Target}/release)
else()
    set(CargoOutDir ${CMAKE_BINARY_DIR}/target/${Target}/debug)
endif()

if (CMAKE_VERBOSE_MAKEFILE)
    set(CargoFlags ${CargoFlags} -vv)
endif()

add_copy_files_to(PythonFiles ${CMAKE_CURRENT_BINARY_DIR}
        ${CMAKE_SOURCE_DIR}/adapter2/codelldb.py
        ${CMAKE_SOURCE_DIR}/adapter2/value.py
        ${CMAKE_SOURCE_DIR}/adapter2/debugger.py
        ${CMAKE_SOURCE_DIR}/formatters/rust.py
)

set(copy_file ${CMAKE_COMMAND} -E copy_if_different)

add_custom_target(codelldb ALL
    DEPENDS codelldb_bin codelldb_lib codelldb_python
)

add_custom_target(codelldb_bin
    COMMAND ${WithEnv} LIBPATH=\"${LibPath}\"
            cargo build --package=codelldb --bin codelldb --features=weak-linkage ${CargoFlags}
    COMMAND ${copy_file} ${CargoOutDir}/codelldb${ExeSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_bin"
    USES_TERMINAL
)

add_custom_target(codelldb_lib
    DEPENDS lldb
    COMMAND ${WithEnv} LIBPATH=\"${LibPath}\" LibPython=${LibPython}
            cargo build --package=codelldb --lib --features=weak-linkage ${CargoFlags}
    COMMAND ${copy_file} ${CargoOutDir}/${DylibPrefix}codelldb${DylibSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_lib"
    USES_TERMINAL
)

add_custom_target(codelldb_python
    DEPENDS lldb ${PythonFiles}
    COMMAND ${WithEnv} LIBPATH=\"${LibPath}\" LibPython=${LibPython}
            cargo build --package=python-impl --lib --features=weak-linkage ${CargoFlags}
    COMMAND ${copy_file} ${CargoOutDir}/${DylibPrefix}codelldb_python${DylibSuffix} ${CMAKE_CURRENT_BINARY_DIR}
    COMMENT "Building codelldb_python"
    USES_TERMINAL
)

set(PATH $ENV{PATH})
string(JOIN ${PathSep} PATH ${PATH} ${LLDB_ROOT}/bin)
string(JOIN ${PathSep} LD_LIBRARY_PATH $ENV{LD_LIBRARY_PATH} ${LLDB_ROOT}/lib)

add_custom_target(cargo_test
    COMMAND ${WithEnv} LIBPATH=\"${LibPath}\" LibPython=${LibPython} PATH=\"${PATH}\" LD_LIBRARY_PATH=\"${LD_LIBRARY_PATH}\" RUST_BACKTRACE=1
            cargo test -p=lldb -p=debug-protocol -p=python-impl -p=codelldb
    USES_TERMINAL
)
