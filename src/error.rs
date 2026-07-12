//! Unified error types for lspz.
//!
//! Uses [`thiserror`] for ergonomic error derivation.

use std::io;

/// Unified error type for the lspz crate.
#[derive(Debug, thiserror::Error)]
pub enum LspzError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("server exited unexpectedly")]
    ServerExited,

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("config error: {0}")]
    Config(String),
}
