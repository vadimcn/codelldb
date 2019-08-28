use clap::{App, Arg, ArgMatches, SubCommand};

fn main() -> Result<(), failure::Error> {
    env_logger::Builder::from_default_env().init();

    let matches = App::new("codelldb")
        .arg(Arg::with_name("port").long("port").takes_value(true))
        .arg(Arg::with_name("multi-session").long("multi-session"))
        .arg(Arg::with_name("preload").long("preload").multiple(true).takes_value(true))
        .arg(Arg::with_name("liblldb").long("liblldb").takes_value(true))
        .arg(Arg::with_name("params").long("params").takes_value(true))
        .subcommand(SubCommand::with_name("terminal-agent").arg(Arg::with_name("port").long("port").takes_value(true)))
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("terminal-agent") {
        terminal_agent(&matches)
    } else {
        debug_server(&matches)
    }
}

fn terminal_agent(matches: &ArgMatches) -> Result<(), failure::Error> {
    use std::io::{Read, Write};
    use std::net;

    let data;
    #[cfg(unix)]
    {
        unsafe {
            let ptr = libc::ttyname(1);
            assert!(!ptr.is_null());
            data = std::ffi::CStr::from_ptr(ptr).to_str()?;
        }
    }
    #[cfg(windows)]
    {
        data = std::process::id();
    }

    let port: u16 = matches.value_of("port").unwrap().parse().unwrap();
    let addr = net::SocketAddr::new(net::Ipv4Addr::new(127, 0, 0, 1).into(), port);
    let mut stream = net::TcpStream::connect(addr)?;
    write!(stream, "{}", data)?;

    let terminal = crossterm::terminal();
    let _ = terminal.clear(crossterm::ClearType::All);

    stream.shutdown(net::Shutdown::Write)?;
    // Wait for the other end to close connection (which will be maintained till the end of
    // the debug session; this prevents terminal shell from stealing debuggee's input form stdin).
    for b in stream.bytes() {
        b?;
    }
    Ok(())
}

fn debug_server(matches: &ArgMatches) -> Result<(), failure::Error> {
    use loading::*;
    use std::mem::transmute;
    use std::path::{Path, PathBuf};

    let multi_session = matches.is_present("multi-session");
    let port = matches.value_of("port").map(|s| s.parse().unwrap()).unwrap_or(0);
    let adapter_params = matches.value_of("params");

    unsafe {
        // Preload anything passed via --preload
        for dylib in matches.values_of("preload").unwrap_or_default() {
            load_library(Path::new(dylib), true)?;
        }

        // Try loading Python dylib.
        // This must be done before loading liblldb, because the latter is weak-linked to libpython.
        let libpython = if cfg!(unix) { "python2.7" } else { "python36" };
        let libpython = get_dylib_filename(libpython);
        match load_library(&Path::new(&libpython), true) {
            Ok(_) => (),
            Err(err) => eprintln!("{:?}", err),
        }

        let mut codelldb_dir = std::env::current_exe()?;
        codelldb_dir.pop();

        // Load liblldb
        let liblldb_path = match matches.value_of("liblldb") {
            Some(path) => PathBuf::from(path),
            None => {
                let mut liblldb_path = codelldb_dir.clone();
                liblldb_path.pop();
                liblldb_path.push("lldb");
                liblldb_path.push(if cfg!(unix) { "lib" } else { "bin" });
                liblldb_path.push(get_dylib_filename("lldb"));
                liblldb_path
            }
        };
        load_library(&liblldb_path, true)?;

        // Load codelldb shared lib
        let mut codelldb_path = codelldb_dir.clone();
        codelldb_path.push(get_dylib_filename("codelldb"));
        let codelldb = load_library(&codelldb_path, false)?;

        // Find codelldb's entry point and call it.
        let entry: unsafe extern "C" fn(u16, bool, Option<&str>) = transmute(find_symbol(codelldb, "entry")?);
        entry(port, multi_session, adapter_params);
    }

    Ok(())
}
