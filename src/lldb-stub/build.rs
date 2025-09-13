use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::iter::FromIterator;
use std::mem;
use std::path::Path;
use std::path::PathBuf;

use cc;
use weaklink_build::exports::dylib_exports;
use weaklink_build::imports::archive_imports;
use weaklink_build::imports::Import;
use weaklink_build::SymbolStub;

type Error = Box<dyn std::error::Error>;

fn main() -> Result<(), Error> {
    let out_dir = env::var("OUT_DIR")?;

    let provider = env::var("LLDB_DYLIB").expect("LLDB_DYLIB");
    let consumer = env::var("DEP_LLDB_GENERATED").expect("DEP_LLDB_GENERATED");

    let exports = dylib_exports(Path::new(&provider))?;
    let imports = archive_imports(Path::new(&consumer))?;

    let imports_str = HashSet::<String>::from_iter(imports.iter().map(|i| i.name.clone()));
    let exports_str = HashSet::<String>::from_iter(exports.iter().map(|e| e.name.clone()));
    let mut common_syms = exports_str.intersection(&imports_str).collect::<HashSet<_>>();

    let mut wl_config = weaklink_build::Config::new("liblldb");

    // Generate optional api groups via SBAPI.toml
    let api_groups = get_api_groups("SBAPI.toml")?;
    for (version, imports) in api_groups {
        for imp in &imports {
            if !common_syms.remove(&imp.name) {
                println!(
                    "cargo:warning=Symbol \"{}\" is declared in {version}, but isn't used by codelldb.",
                    imp.name
                );
            }
        }
        let symbols: Vec<_> = imports.iter().map(|e| SymbolStub::new(&e.name)).collect();
        wl_config.add_symbol_group(version.as_str(), symbols)?;
    }

    // Emit the rest of common symbols as base group.
    let symbols: Vec<_> = exports
        .into_iter()
        .filter(|e| common_syms.contains(&e.name))
        .map(|e| SymbolStub::new(&e.name))
        .collect();
    wl_config.add_symbol_group("base", symbols)?;

    let generated = Path::new(&out_dir).join("generated.rs");
    let mut f = File::create(&generated)?;
    wl_config.generate_source(&mut f);
    mem::drop(f);

    println!("cargo:rerun-if-changed={}", provider);
    println!("cargo:rerun-if-changed={}", consumer);
    println!("cargo:rerun-if-changed=SBAPI.toml");

    Ok(())
}

fn get_api_groups(manifest: &str) -> Result<HashMap<String, Vec<Import>>, Error> {
    use toml::Value;

    let cfg = fs::read_to_string(manifest)?;
    let table: Value = toml::from_str(&cfg)?;
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    let mut api_groups = HashMap::new();

    for (version, section) in table.as_table().unwrap() {
        let cpp_path = out_dir.join(format!("probe_{version}.cpp"));
        let mut cpp = fs::File::create(&cpp_path)?;
        writeln!(cpp, "#define LLDB_API")?; // On Windows we want the "static" symbols
        writeln!(cpp, "#include <lldb/API/LLDB.h>")?;
        writeln!(cpp, "using namespace lldb;")?;

        let mut idx = 0;
        for (class, methods) in section.as_table().unwrap() {
            for method in methods.as_array().unwrap() {
                let (name, params, qual) = split_method_sig(method.as_str().unwrap());
                if name == class {
                    writeln!(cpp, "auto c{idx} = {class}{params};")?; // constructor
                } else {
                    writeln!(cpp, "auto ({class}::* p{idx}){params}{qual} = &{class}::{name};")?;
                }
                idx += 1;
            }
        }

        drop(cpp);
        let mut build = cc::Build::new();
        build.cargo_metadata(false);
        build.cpp(true).std("c++17");
        for dir in env::var("LLDB_INCLUDE")?.split(';') {
            build.include(dir);
        }
        build.file(&cpp_path).out_dir(&out_dir);
        build.compile(&format!("probe_{version}"));

        let is_windows = env::var("TARGET")?.contains("msvc");
        let libname = if !is_windows { format!("libprobe_{version}.a") } else { format!("probe_{version}.lib") };
        let imports = archive_imports(&out_dir.join(&libname))?;
        // Filter out unrelated imports such as __cxa_atexit
        let lldb_imports: Vec<_> = imports.into_iter().filter(|imp| imp.name.contains("lldb")).collect();
        api_groups.insert(version.to_owned(), lldb_imports);
    }
    Ok(api_groups)
}

// "GetProcessInfoAtIndex(uint32_t, SBProcessInfo&) const" ->  ("GetProcessInfoAtIndex", "(uint32_t, SBProcessInfo&)", " const")
fn split_method_sig(sig: &str) -> (&str, &str, &str) {
    let a = sig.find('(').unwrap();
    let b = sig.rfind(')').unwrap() + 1;
    (&sig[..a], &sig[a..b], &sig[b..])
}
