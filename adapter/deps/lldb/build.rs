use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let weak_linkage = match env::var("CARGO_FEATURE_WEAK_LINKAGE") {
        Ok(_) => true,
        Err(_) => false,
    };

    // Generate C++ bindings
    let mut build_config = cpp_build::Config::new();
    build_config.include("include");
    build_config.debug(true);
    if weak_linkage {
        build_config.cpp_link_stdlib(None);
    }

    build_config.build("src/lldb.rs");
    for entry in fs::read_dir("src").unwrap() {
        println!("cargo:rerun-if-changed={}", entry.unwrap().path().display());
    }

    if target_os == "windows" {
        strong_linkage();
    } else {
        if weak_linkage {
            if target_os == "macos" {
                println!("cargo:rustc-cdylib-link-arg=-undefined");
                println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
            }
        } else {
            strong_linkage();
        }
    }
}

fn strong_linkage() {
    // Find CMakeCache
    let mut path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let cmakecache = loop {
        let f = path.with_file_name("CMakeCache.txt");
        if f.is_file() {
            break f;
        }
        if !path.pop() {
            println!("cargo:warning=Could not find CMakeCache.txt");
            return;
        }
    };
    println!("cargo:rerun-if-changed={}", cmakecache.display());
    let config = parse_cmakecache(&cmakecache);

    if let Some(value) = config.get("LLDB_LinkSearch") {
        for path in value.split_terminator(';') {
            println!("cargo:rustc-link-search=native={}", path);
        }
    } else {
        println!("cargo:warning=LLDB_LinkSearch not set");
    }

    if let Some(value) = config.get("LLDB_LinkDylib") {
        for path in value.split_terminator(';') {
            println!("cargo:rustc-link-lib=dylib={}", path);
        }
    } else {
        println!("cargo:warning=LLDB_LinkDylib not set");
    }
}

fn parse_cmakecache(cmakecache: &Path) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let reader = BufReader::new(File::open(cmakecache).expect("Open file"));
    let kvregex = Regex::new("(?mx) ^ ([A-Za-z_]+) : (STRING|FILEPATH|BOOL|STATIC|INTERNAL) = (.*)").unwrap();

    for line in reader.lines() {
        let line = line.unwrap();
        if let Some(captures) = kvregex.captures(&line) {
            let key = captures.get(1).expect("Invalid format").as_str();
            let value = captures.get(3).expect("Invalid format").as_str();
            result.insert(key.into(), value.into());
        }
    }
    result
}
