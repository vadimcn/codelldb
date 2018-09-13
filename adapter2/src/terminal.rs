use std::io::{self, BufRead};
use std::net;

pub struct Terminal {
    connection: net::TcpStream,
    tty_name: String,
}

impl Terminal {
    pub fn create<F>(run_in_terminal: F) -> Result<Self, io::Error>
    where
        F: FnOnce(Vec<String>),
    {
        let mut listener = net::TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;

        // Opens TCP connection, send output of `tty`, wait till the socket gets closed from our end
        let args = vec![
            "/bin/bash".to_owned(),
            "-c".to_owned(),
            format!("exec 3<>/dev/tcp/127.0.0.1/{}; tty >&3; clear; read <&3", addr.port()),
        ];
        run_in_terminal(args);

        let (stream, _) = listener.accept()?;
        let stream2 = stream.try_clone()?;

        let mut reader = io::BufReader::new(stream);
        let mut tty_name = String::new();
        reader.read_line(&mut tty_name)?;

        Ok(Terminal {
            connection: stream2,
            tty_name: tty_name.trim().to_owned(),
        })
    }

    pub fn tty_name(&self) -> &str {
        &self.tty_name
    }
}
