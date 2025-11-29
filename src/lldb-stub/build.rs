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
    let cfg = fs::read_to_string(manifest)?;
    let table = cfg.parse::<toml::Table>()?;
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    let mut api_groups = HashMap::new();

    for (version, signatures) in table {
        let cpp_path = out_dir.join(format!("probe_{version}.cpp"));
        let mut cpp = fs::File::create(&cpp_path)?;
        writeln!(cpp, "#define LLDB_API")?; // On Windows we want the "static" symbols
        writeln!(cpp, "#include <lldb/API/LLDB.h>")?;
        writeln!(cpp, "using namespace lldb;")?;

        for (idx, signature) in signatures.as_array().expect("list").iter().enumerate() {
            let (class, name, args, qual) = split_fn_signature(signature.as_str().expect("string"));
            if let Some(class) = class {
                writeln!(cpp, "auto ({class}::* p{idx}){args}{qual} = &{class}::{name};")?;
            } else {
                writeln!(cpp, "auto (*p{idx}){args}{qual} = &{name};")?; // Standalone function or static method.
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

// Splits function signature into: (<class name if any>, <function name>, <arguments>, <qualifiers>)
//
// "lldb::SBAddressRangeList::GetSize() const"
//    -> (Some("lldb::SBAddressRangeList", "GetSize", "()", " const")
//  "static lldb::SBDebugger::GetDiagnosticFromEvent(const lldb::SBEvent &event)"
//    -> (None, "lldb::SBDebugger::GetDiagnosticFromEvent", "(const lldb::SBEvent &event)", "")

fn split_fn_signature(mut signarure: &str) -> (Option<&str>, &str, &str, &str) {
    let mut is_static = false;
    if signarure.starts_with("static ") {
        is_static = true;
        signarure = &signarure[7..];
    }
    let i = signarure.find('(').unwrap();
    let j = signarure.rfind(')').unwrap() + 1;
    let name = &signarure[..i];
    let args = &signarure[i..j];
    let qual = &signarure[j..];

    if is_static {
        (None, name, args, qual)
    } else {
        let i = name.rfind("::").unwrap();
        (Some(&name[..i]), &name[i + 2..], args, qual)
    }
}
