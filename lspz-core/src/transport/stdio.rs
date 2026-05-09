//! Stdio transport for LSP communication.
//!
//! Spawns a child process and communicates over its stdin/stdout.

use std::process::Stdio;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::error::LspzError;

/// Transport over a child process's stdio.
///
/// Spawns the backend LSP server and wraps its stdin/stdout in an async I/O layer.
pub struct StdioTransport {
    child: Option<Child>,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl StdioTransport {
    /// Spawn a new process and connect to its stdio.
    ///
    /// The `cmd` is split on whitespace into program + arguments.
    pub fn spawn(cmd: &str) -> Result<Self, LspzError> {
        let parts = shell_words::split(cmd)
            .map_err(|e| LspzError::Config(format!("failed to parse command '{cmd}': {e}")))?;

        let mut iter = parts.into_iter();
        let program = iter
            .next()
            .ok_or_else(|| LspzError::Config("empty backend command".into()))?;
        let args: Vec<String> = iter.collect();

        let mut child = Command::new(&program)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Forward server stderr for debugging
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                LspzError::Config(format!("failed to spawn '{program}': {e}"))
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspzError::Config("failed to open stdin on child process".into()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspzError::Config("failed to open stdout on child process".into()))?;

        Ok(Self {
            child: Some(child),
            stdin,
            reader: BufReader::new(stdout),
        })
    }

    /// Check if the child process has exited.
    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, LspzError> {
        match self.child.as_mut() {
            Some(child) => Ok(child.try_wait()?),
            None => Ok(None),
        }
    }
}

#[async_trait::async_trait]
impl super::Transport for StdioTransport {
    async fn receive(&mut self) -> Result<Vec<u8>, LspzError> {
        let mut buffer = Vec::with_capacity(4096);

        // Read headers until we find \r\n\r\n
        loop {
            let byte = self.reader.read_u8().await.map_err(|e| {
                // Map EOF to ServerExited
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    LspzError::ServerExited
                } else {
                    LspzError::Io(e)
                }
            })?;
            buffer.push(byte);

            // Check for \r\n\r\n terminator at end of buffer
            if buffer.len() >= 4 && buffer[buffer.len() - 4..] == [b'\r', b'\n', b'\r', b'\n'] {
                break;
            }
        }

        let header = std::str::from_utf8(&buffer)
            .map_err(|_| LspzError::Protocol("header is not valid UTF-8".into()))?;

        // Parse Content-Length
        let content_length = parse_content_length_from_header(header)?;

        // Read the body
        let mut body = vec![0u8; content_length as usize];
        self.reader.read_exact(&mut body).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                LspzError::ServerExited
            } else {
                LspzError::Io(e)
            }
        })?;

        // Reconstruct: header + body
        let mut frame = Vec::with_capacity(buffer.len() + body.len());
        frame.extend_from_slice(&buffer);
        frame.extend_from_slice(&body);
        Ok(frame)
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), LspzError> {
        self.stdin.write_all(data).await?;
        self.stdin.flush().await?;
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Kill the child process on drop
            let _ = child.start_kill();
        }
    }
}

/// Parse Content-Length from the accumulated header bytes.
fn parse_content_length_from_header(header: &str) -> Result<u64, LspzError> {
    for line in header.lines() {
        let line = line.trim();
        if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
            let value = value.trim();
            return value
                .parse::<u64>()
                .map_err(|_| LspzError::Protocol(format!("invalid Content-Length: {value}")));
        }
    }
    Err(LspzError::Protocol("missing Content-Length header".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_length() {
        let header = "Content-Length: 47\r\n\r\n";
        assert_eq!(parse_content_length_from_header(header).unwrap(), 47);
    }

    #[test]
    fn test_parse_content_length_case_insensitive() {
        let header = "content-length: 128\r\n\r\n";
        assert_eq!(parse_content_length_from_header(header).unwrap(), 128);
    }

    #[test]
    fn test_parse_content_length_whitespace() {
        let header = "Content-Length:   99   \r\n\r\n";
        assert_eq!(parse_content_length_from_header(header).unwrap(), 99);
    }

    #[test]
    fn test_missing_content_length() {
        let header = "\r\n\r\n";
        match parse_content_length_from_header(header) {
            Err(LspzError::Protocol(msg)) => assert!(msg.contains("missing")),
            _ => panic!("expected Protocol error"),
        }
    }

    #[test]
    fn test_invalid_content_length() {
        let header = "Content-Length: abc\r\n\r\n";
        match parse_content_length_from_header(header) {
            Err(LspzError::Protocol(msg)) => assert!(msg.contains("invalid")),
            _ => panic!("expected Protocol error"),
        }
    }
}
