use log::{debug, error, info};
use tokio::io;
use tokio::prelude::*;

pub struct StdioChannel {
    stdin: io::Stdin,
    stdout: io::Stdout,
}

pub fn create() -> StdioChannel {
    StdioChannel {
        stdin: io::stdin(),
        stdout: io::stdout(),
    }
}

impl io::Read for StdioChannel {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        debug!("stdin.read");
        self.stdin.read(buf)
    }
}

impl io::Write for StdioChannel {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        debug!("stdout.write {:?}", buf);
        self.stdout.write(buf)
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        let x = self.stdout.flush();
        debug!("stdout.flush {:?}", x);
        x
    }
}

impl io::AsyncRead for StdioChannel {}

impl io::AsyncWrite for StdioChannel {
    fn shutdown(&mut self) -> Result<Async<()>, io::Error> {
        debug!("stdout.shutdown");
        self.stdout.shutdown()
    }
    fn poll_write(&mut self, buf: &[u8]) -> Result<Async<usize>, io::Error> {
        let x = self.stdout.poll_write(buf);
        debug!("stdout.poll_write {:?}", x);
        x
    }
    fn poll_flush(&mut self) -> Result<Async<()>, io::Error> {
        let x = self.stdout.poll_flush();
        debug!("stdout.poll_flush {:?}", x);
        x
    }
    fn write_buf<B>(&mut self, buf: &mut B) -> Result<Async<usize>, io::Error>
    where
        B: bytes::buf::Buf,
    {
        let x = self.stdout.write_buf(buf);
        debug!("stdout.write_buf {:?}", x);
        x
    }
}
