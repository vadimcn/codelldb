set(LLVM_TRIPLE x86_64-unknown-linux-gnu)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)
set(CMAKE_C_COMPILER_TARGET ${LLVM_TRIPLE})
set(CMAKE_CXX_COMPILER_TARGET ${LLVM_TRIPLE})
set(CMAKE_CXX_FLAGS_INIT -stdlib=libc++)
set(CMAKE_EXE_LINKER_FLAGS_INIT "-fuse-ld=lld")
set(CMAKE_MODULE_LINKER_FLAGS_INIT "-fuse-ld=lld")
set(CMAKE_SHARED_LINKER_FLAGS_INIT "-fuse-ld=lld")
set(CMAKE_STRIP llvm-strip)
