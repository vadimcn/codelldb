use clap::{App, Arg, ArgMatches, SubCommand};

type Error = Box<dyn std::error::Error>;

mod find_python;

fn main() -> Result<(), Error> {
    env_logger::Builder::from_default_env().init();

    let matches = App::new("codelldb")
        .arg(Arg::with_name("port").long("port").takes_value(true))
        .arg(Arg::with_name("multi-session").long("multi-session"))
        .arg(Arg::with_name("preload").long("preload").multiple(true).takes_value(true))
        .arg(Arg::with_name("libpython").long("libpython").takes_value(true))
        .arg(Arg::with_name("liblldb").long("liblldb").takes_value(true))
        .arg(Arg::with_name("params").long("params").takes_value(true))
        .subcommand(SubCommand::with_name("terminal-agent").arg(Arg::with_name("port").long("port").takes_value(true)))
        .subcommand(SubCommand::with_name("find-python"))
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("terminal-agent") {
        terminal_agent(&matches)
    } else if let Some(_) = matches.subcommand_matches("find-python") {
        find_python()
    } else {
        debug_server(&matches)
    }
}

fn terminal_agent(matches: &ArgMatches) -> Result<(), Error> {
    use std::io::{Read, Write};
    use std::net;

    fn clear_screen() {
        let terminal = crossterm::terminal();
        drop(terminal.clear(crossterm::ClearType::All));
    }

    #[cfg(unix)]
    fn purge_stdin() {
        use std::os::unix::io::AsRawFd;
        drop(termios::tcflush(std::io::stdin().as_raw_fd(), termios::TCIFLUSH));
    }
    #[cfg(windows)]
    fn purge_stdin() {
        use std::os::windows::io::AsRawHandle;
        unsafe {
            winapi::um::wincon::FlushConsoleInputBuffer(std::io::stdin().as_raw_handle());
        }
    }

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

    clear_screen();

    stream.shutdown(net::Shutdown::Write)?;
    // Wait for the other end to close connection (which will be maintained till the end of
    // the debug session; this prevents terminal shell from stealing debuggee's input form stdin).
    for b in stream.bytes() {
        b?;
    }

    // Clear out any unread input buffered in stdin, so it doesn't get read by the shell.
    purge_stdin();

    Ok(())
}

fn find_python() -> Result<(), Error> {
    match find_python::find_python() {
        Ok(path) => {
            println!("{}", path.display());
            return Ok(());
        }
        Err(err) => {
            eprintln!("{}", err);
            Err(err)
        }
    }
}

fn debug_server(matches: &ArgMatches) -> Result<(), Error> {
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

        // Try loading libpython
        // This must be done before loading liblldb, because the latter is weak-linked to libpython.
        if let Some(libpython) = matches.value_of("libpython") {
            match load_library(&Path::new(&libpython), true) {
                Ok(_) => (),
                Err(err) => eprintln!("{}", err),
            }
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
                liblldb_path.push(DYLIB_SUBDIR);
                liblldb_path.push(format!("liblldb.{}", DYLIB_EXTENSION));
                liblldb_path
            }
        };
        load_library(&liblldb_path, true)?;

        // Load codelldb shared lib
        let mut codelldb_path = codelldb_dir.clone();
        codelldb_path.push(format!("{}codelldb.{}", DYLIB_PREFIX, DYLIB_EXTENSION));
        let codelldb = load_library(&codelldb_path, false)?;

        // Find codelldb's entry point and call it.
        let entry: unsafe extern "C" fn(u16, bool, Option<&str>) = transmute(find_symbol(codelldb, "entry")?);
        entry(port, multi_session, adapter_params);
    }

    Ok(())
}
