use clap::{App, Arg, SubCommand};
use log::info;

type Error = Box<dyn std::error::Error>;

mod terminal_agent;

fn main() -> Result<(), Error> {
    env_logger::Builder::from_default_env().init();

    let matches = App::new("codelldb")
        .arg(Arg::with_name("preload").long("preload").multiple(true).takes_value(true))
        .arg(Arg::with_name("liblldb").long("liblldb").takes_value(true))
        .arg(Arg::with_name("port").long("port").takes_value(true))
        .arg(Arg::with_name("connect").long("connect").takes_value(true))
        .arg(Arg::with_name("auth-token").long("auth-token").takes_value(true))
        .arg(Arg::with_name("multi-session").long("multi-session"))
        .arg(Arg::with_name("settings").long("settings").takes_value(true))
        .subcommand(
            SubCommand::with_name("terminal-agent").arg(Arg::with_name("connect").long("connect").takes_value(true)),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("terminal-agent") {
        terminal_agent::terminal_agent(&matches)
    } else {
        use std::path::PathBuf;

        #[cfg(unix)]
        pub const DYLIB_SUBDIR: &str = "lib";
        #[cfg(windows)]
        pub const DYLIB_SUBDIR: &str = "bin";

        #[cfg(target_os = "linux")]
        pub const DYLIB_EXTENSION: &str = "so";
        #[cfg(target_os = "macos")]
        pub const DYLIB_EXTENSION: &str = "dylib";
        #[cfg(target_os = "windows")]
        pub const DYLIB_EXTENSION: &str = "dll";

        // Load liblldb
        let mut codelldb_dir = std::env::current_exe()?;
        codelldb_dir.pop();
        let liblldb_path = match matches.value_of("liblldb") {
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

        codelldb::debug_server(&matches)
    }
}
