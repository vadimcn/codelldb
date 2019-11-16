use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let weak_linkage = match env::var("CARGO_FEATURE_WEAK_LINKAGE") {
        Ok(_) => true,
        Err(_) => false,
    };
    let no_link_cpp_stdlib = match env::var("CARGO_FEATURE_NO_LINK_CPP_STDLIB") {
        Ok(_) => true,
        Err(_) => false,
    };

    let mut build_config = cpp_build::Config::new();
    build_config.include("include");
    if no_link_cpp_stdlib {
        build_config.cpp_link_stdlib(None);
    }
    build_config.build("src/lldb.rs");

    println!("cargo:rerun-if-env-changed=LIBPATH");
    if let Ok(libpath) = env::var("LIBPATH") {
        for dir in libpath.split(';') {
            println!("cargo:rustc-link-search=native={}", dir);
        }
    }

    if target_os == "windows" {
        println!("cargo:rustc-link-lib=dylib=liblldb");
        link_python(); // liblldb depends on Python too
    } else {
        if weak_linkage {
            if target_os == "macos" {
                println!("cargo:rustc-cdylib-link-arg=-undefined");
                println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
            }
        } else {
            println!("cargo:rustc-link-lib=dylib=lldb");
            link_python();
        }
    }
}

fn link_python() {
    println!("cargo:rerun-if-env-changed=LibPython");
    if let Ok(libpython) = env::var("LibPython") {
        println!("cargo:rustc-link-lib=dylib={}", libpython);
    }
}
