use std::{env, fs, path::Path};

pub type Error = Box<dyn std::error::Error>;

fn main() -> Result<(), Error> {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let weak_linkage = match env::var("CARGO_FEATURE_WEAK_LINKAGE") {
        Ok(_) => true,
        Err(_) => false,
    };

    // Rebuild if any source files change
    rerun_if_changed_in(Path::new("src"))?;

    let mut build_config = cpp_build::Config::new();

    if weak_linkage {
        build_config.cpp_set_stdlib(None);
    } else {
        // This branch is used when building test runners
        set_rustc_link_search();
        set_dylib_search_path();
        if target_os == "windows" {
            println!("cargo:rustc-link-lib=dylib=liblldb");
        } else {
            build_config.cpp_set_stdlib(Some("c++"));
            println!("cargo:rustc-link-lib=dylib=lldb");
            if target_os == "linux" {
                // Require all symbols to be defined in test runners
                println!("cargo:rustc-link-arg=--no-undefined");
            }
        }
    }

    // Generate C++ bindings
    build_config.include("include");
    build_config.build("src/lib.rs");

    Ok(())
}

fn rerun_if_changed_in(dir: &Path) -> Result<(), Error> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        } else {
            rerun_if_changed_in(&entry.path())?;
        }
    }
    Ok(())
}

fn set_rustc_link_search() {
    if let Ok(value) = env::var("CODELLDB_LIB_PATH") {
        for path in value.split_terminator(';') {
            println!("cargo:rustc-link-search=native={}", path);
        }
    }
}

fn set_dylib_search_path() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    if let Ok(value) = env::var("CODELLDB_LIB_PATH") {
        if target_os == "linux" {
            let prev = env::var("LD_LIBRARY_PATH").unwrap_or_default();
            println!("cargo:rustc-env=LD_LIBRARY_PATH={}:{}", prev, value.replace(";", ":"));
        } else if target_os == "macos" {
            println!("cargo:rustc-env=DYLD_FALLBACK_LIBRARY_PATH={}", value.replace(";", ":"));
        } else if target_os == "windows" {
            println!("cargo:rustc-env=PATH={};{}", env::var("PATH").unwrap(), value);
        }
    }
}
