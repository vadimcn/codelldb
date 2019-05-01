extern crate cpp_build;

fn main() {
    cpp_build::Config::new().include("include").build("src/lldb.rs");
}
