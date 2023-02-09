use std::env;

fn main() {
    set_dylib_search_path();
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
