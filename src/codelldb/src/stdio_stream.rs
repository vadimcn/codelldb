use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub struct StdioStream {
    std_in: tokio::io::Stdin,
    std_out: tokio::io::Stdout,
}

impl AsyncRead for StdioStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().std_in).poll_read(cx, buf)
    }
}

impl AsyncWrite for StdioStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.get_mut().std_out).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().std_out).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().std_out).poll_shutdown(cx)
    }
}

impl StdioStream {
    pub fn new() -> Self {
        StdioStream {
            std_in: tokio::io::stdin(),
            std_out: tokio::io::stdout(),
        }
    }
}
