//! TCP transport for LSP communication.
//!
//! Connects to a remote LSP server over TCP with standard Content-Length framing.

use tokio::io::{AsyncWriteExt, BufReader, ReadHalf, WriteHalf};

use crate::error::LspzError;

use super::Transport;
use super::framing;

/// Transport over a raw TCP socket.
///
/// Connects to a remote LSP server (e.g. `rust-analyzer --port 2087`)
/// and wraps the socket in Content-Length framed I/O.
pub struct TcpTransport {
    writer: WriteHalf<tokio::net::TcpStream>,
    reader: BufReader<ReadHalf<tokio::net::TcpStream>>,
}

impl TcpTransport {
    /// Connect to a TCP address.
    ///
    /// `addr` should be in the form `host:port` (e.g. `"localhost:2087"`).
    pub async fn connect(addr: &str) -> Result<Self, LspzError> {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        let (read_half, write_half) = tokio::io::split(stream);
        Ok(Self {
            writer: write_half,
            reader: BufReader::new(read_half),
        })
    }
}

#[async_trait::async_trait]
impl Transport for TcpTransport {
    async fn receive(&mut self) -> Result<Vec<u8>, LspzError> {
        framing::read_frame(&mut self.reader).await
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), LspzError> {
        self.writer.write_all(data).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Echo server that responds with Content-Length framed messages.
    async fn echo_server(addr: &str) -> Result<(), std::io::Error> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let (mut stream, _) = listener.accept().await?;
        let mut buf = vec![0u8; 4096];
        loop {
            let n = stream.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            stream.write_all(&buf[..n]).await?;
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_tcp_send_receive() {
        let addr = "127.0.0.1:19876";
        let server_handle = tokio::spawn(echo_server(addr));

        // Give the server time to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let mut transport = TcpTransport::connect(addr).await.unwrap();

        let frame = b"Content-Length: 5\r\n\r\nhello";
        transport.send(frame).await.unwrap();
        let result = transport.receive().await.unwrap();
        assert_eq!(result, frame);

        // Clean up
        drop(transport);
        server_handle.abort();
    }
}
