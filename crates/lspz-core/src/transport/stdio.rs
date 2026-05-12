//! Stdio transport for LSP communication.
//!
//! Spawns a child process and communicates over its stdin/stdout.

use std::process::Stdio;

use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::error::LspzError;

use super::framing;

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
    /// `extra_args` are appended after the parsed command-line arguments
    /// (e.g. arguments captured from the `--` separator on the CLI).
    pub fn spawn(cmd: &str, extra_args: &[String]) -> Result<Self, LspzError> {
        let mut parts = shell_words::split(cmd)
            .map_err(|e| LspzError::Config(format!("failed to parse command '{cmd}': {e}")))?;
        parts.extend_from_slice(extra_args);

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
        framing::read_frame(&mut self.reader).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_length() {
        let header = "Content-Length: 47\r\n\r\n";
        assert_eq!(framing::parse_content_length(header).unwrap(), 47);
    }

    #[test]
    fn test_parse_content_length_case_insensitive() {
        let header = "content-length: 128\r\n\r\n";
        assert_eq!(framing::parse_content_length(header).unwrap(), 128);
    }

    #[test]
    fn test_parse_content_length_whitespace() {
        let header = "Content-Length:   99   \r\n\r\n";
        assert_eq!(framing::parse_content_length(header).unwrap(), 99);
    }

    #[test]
    fn test_missing_content_length() {
        let header = "\r\n\r\n";
        match framing::parse_content_length(header) {
            Err(LspzError::Protocol(msg)) => assert!(msg.contains("missing")),
            _ => panic!("expected Protocol error"),
        }
    }

    #[test]
    fn test_invalid_content_length() {
        let header = "Content-Length: abc\r\n\r\n";
        match framing::parse_content_length(header) {
            Err(LspzError::Protocol(msg)) => assert!(msg.contains("invalid")),
            _ => panic!("expected Protocol error"),
        }
    }
}
