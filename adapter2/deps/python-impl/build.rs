use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let weak_linkage = match env::var("CARGO_FEATURE_WEAK_LINKAGE") {
        Ok(_) => true,
        Err(_) => false,
    };

    println!("cargo:rerun-if-env-changed=LIBPATH");
    if let Ok(libpath) = env::var("LIBPATH") {
        for dir in libpath.split(';') {
            println!("cargo:rustc-link-search=native={}", dir);
        }
    }

    if target_os == "windows" {
        link_python();
    } else {
        if weak_linkage {
            if target_os == "macos" {
                println!("cargo:rustc-cdylib-link-arg=-undefined");
                println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
            }
        } else {
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
