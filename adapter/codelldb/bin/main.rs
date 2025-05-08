use std::path::PathBuf;

use clap::Parser;
use log::info;

type Error = Box<dyn std::error::Error>;

fn main() -> Result<(), Error> {
    env_logger::Builder::from_default_env().init();

    let cli = codelldb::Cli::parse();

    #[cfg(unix)]
    const DYLIB_SUBDIR: &str = "lib";
    #[cfg(windows)]
    const DYLIB_SUBDIR: &str = "bin";

    #[cfg(target_os = "linux")]
    const DYLIB_EXTENSION: &str = "so";
    #[cfg(target_os = "macos")]
    const DYLIB_EXTENSION: &str = "dylib";
    #[cfg(target_os = "windows")]
    const DYLIB_EXTENSION: &str = "dll";

    // Load liblldb
    let mut codelldb_dir = std::env::current_exe()?;
    codelldb_dir.pop();
    let liblldb_path = match &cli.liblldb {
        Some(path) => PathBuf::from(path),
        None => {
            let mut liblldb_path = codelldb_dir.clone();
            liblldb_path.pop();
            liblldb_path.push("lldb");
            liblldb_path.push(DYLIB_SUBDIR);
            liblldb_path.push(format!("liblldb.{}", DYLIB_EXTENSION));
            liblldb_path
        }
    };

    lldb_stub::liblldb.load_from(&liblldb_path).unwrap();
    match lldb_stub::base.resolve() {
        Ok(token) => token.mark_permanent(),
        Err(err) => {
            log::error!("Unable to resolve liblldb symbol: {}", err);
            return Err(err);
        }
    }
    info!(
        "Loaded {liblldb_path:?}, version=\"{}\"",
        lldb::SBDebugger::version_string()
    );

    codelldb::debug_server(&cli)
}
