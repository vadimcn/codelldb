extern crate cpp_build;

fn main() {
    cpp_build::Config::new().include("include").build("src/lldb.rs");

    #[cfg(target_os = "linux")]
    {
        // println!("cargo:rustc-link-search={}", "/usr/lib/llvm-6.0/lib");
        // println!("cargo:rustc-link-lib={}", "lldb-6.0");
    }
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-search={}/lib", std::env::var("LLDB_ROOT").unwrap());
        println!("cargo:rustc-link-lib={}", "lldb");
    }
    #[cfg(windows)]
    {
        println!("cargo:rustc-link-search={}/lib", std::env::var("LLDB_ROOT").unwrap());
        println!("cargo:rustc-link-lib={}", "liblldb");
    }
}
