//! Transport abstraction.
//!
//! Defines the I/O trait that all transports must implement.
//!
//! ## Available Transports
//!
//! | Transport | Protocol | Feature Flag | Status |
//! |-----------|----------|-------------|--------|
//! | [`StdioTransport`] | Child process stdio | always | ✅ |
//! | [`TcpTransport`] | TCP socket | always | ✅ |
//! | [`WsTransport`] | WebSocket | `transport-websocket` | ✅ |
//! | [`MockTransport`] | In-memory FIFO | always (testing) | ✅ |
//!
//! ## Architecture
//!
//! [MermaidChart:docs/src/diagrams/transport-architecture.mmd]

pub(crate) mod framing;
pub mod mock;
pub mod stdio;
pub mod tcp;

/// This module is only available with the `transport-websocket` feature.
#[cfg(feature = "transport-websocket")]
pub mod websocket;

use std::process::ExitStatus;

use crate::error::LspzError;

/// Abstract I/O channel for LSP communication.
///
/// All LSP message I/O (regardless of transport protocol) is defined by this trait.
/// Implementations handle Content-Length framing internally and expose raw framed bytes.
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
