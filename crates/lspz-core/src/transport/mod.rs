//! Transport abstraction.
//!
//! Defines the I/O trait that all transports must implement.

pub mod mock;
pub mod stdio;

use std::process::ExitStatus;

use crate::error::LspzError;

/// Abstract I/O channel for LSP communication.
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Receive one raw LSP message (Content-Length framed).
    async fn receive(&mut self) -> Result<Vec<u8>, LspzError>;

    /// Send raw bytes to the LSP server/client.
    async fn send(&mut self, data: &[u8]) -> Result<(), LspzError>;

    /// Check whether the underlying process has exited.
    ///
    /// Returns `Ok(None)` by default for non-process transports (mock, TCP, WebSocket).
    fn try_wait(&mut self) -> Result<Option<ExitStatus>, LspzError> {
        Ok(None)
    }
}
