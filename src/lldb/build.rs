use std::{env, fs, path::Path};

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let no_link_args = env::var("CARGO_FEATURE_NO_LINK_ARGS").is_ok();

    // Rebuild if any of the source files change
    rerun_if_changed_in(Path::new("src"));

    let mut build_config = cpp_build::Config::new();

    for dir in env::var("LLDB_INCLUDE").unwrap().split(';') {
        build_config.include(dir);
    }

    if no_link_args {
        build_config.cpp_set_stdlib(None);
    } else {
        // This branch is used when building unit tests, etc.
        if target_os == "linux" {
            build_config.cpp_set_stdlib(Some("c++"));
            println!("cargo:rustc-link-arg=--no-undefined");
        } else if target_os == "macos" {
            build_config.cpp_set_stdlib(Some("c++"));
        }
    }

    // Generate C++ bindings
    build_config.build("src/lib.rs");

    let generated_lib = Path::new(&env::var("OUT_DIR").unwrap()).join(if cfg!(unix) {
        "librust_cpp_generated.a"
    } else {
        "rust_cpp_generated.lib"
    });
    println!("cargo:GENERATED={}", generated_lib.display());
}

fn rerun_if_changed_in(dir: &Path) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        } else {
            rerun_if_changed_in(&entry.path());
        }
    }
}
