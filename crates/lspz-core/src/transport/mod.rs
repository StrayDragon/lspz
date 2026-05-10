//! Transport abstraction.
//!
//! Defines the I/O trait that all transports must implement.

pub mod stdio;

use crate::error::LspzError;

/// Abstract I/O channel for LSP communication.
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Receive one raw LSP message (Content-Length framed).
    async fn receive(&mut self) -> Result<Vec<u8>, LspzError>;

    /// Send raw bytes to the LSP server/client.
    async fn send(&mut self, data: &[u8]) -> Result<(), LspzError>;
}
