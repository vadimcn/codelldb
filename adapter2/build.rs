use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let weak_linkage = match env::var("CARGO_FEATURE_WEAK_LINKAGE") {
        Ok(_) => true,
        Err(_) => false,
    };

    if weak_linkage {
        if target_os == "linux" {
            println!("cargo:rustc-cdylib-link-arg=-Wl,-Bstatic");
            println!("cargo:rustc-cdylib-link-arg=-lstdc++");
            println!("cargo:rustc-cdylib-link-arg=-Wl,-Bdynamic");
        } else if target_os == "macos" {
            println!("cargo:rustc-cdylib-link-arg=-undefined");
            println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
        }
    }
}
