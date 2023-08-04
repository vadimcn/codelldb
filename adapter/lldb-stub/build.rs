use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::iter::FromIterator;
use std::mem;
use std::path::Path;

use weaklink_build::exports::dylib_exports;
use weaklink_build::imports::archive_imports;
use weaklink_build::SymbolStub;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let provider = env::var("LLDB_DYLIB").unwrap();
    let consumer = env::var("DEP_LLDB_GENERATED").unwrap();

    let exports = dylib_exports(Path::new(&provider)).unwrap();
    let imports = archive_imports(Path::new(&consumer)).unwrap();

    let imports_str = HashSet::<String>::from_iter(imports.iter().map(|i| {
        // Windows prefixes dll imports with __imp_
        match i.name.strip_prefix("__imp_") {
            Some(name) => name,
            None => &i.name,
        }
        .to_string()
    }));
    let exports_str = HashSet::<String>::from_iter(exports.iter().map(|e| e.name.clone()));
    let common = exports_str.intersection(&imports_str).collect::<HashSet<_>>();
    let symbols = exports
        .into_iter()
        .filter(|e| common.contains(&e.name))
        .map(|e| SymbolStub::new(&e.name))
        .collect::<Vec<_>>();

    let mut config = weaklink_build::Config::new("liblldb");
    config.add_symbol_group("base_api", symbols).unwrap();

    let generated = Path::new(&out_dir).join("generated.rs");
    let mut f = File::create(&generated).unwrap();
    config.generate_source(&mut f);
    mem::drop(f);

    println!("cargo:rerun-if-changed={}", provider);
    println!("cargo:rerun-if-changed={}", consumer);
}
