//! # lspz
//!
//! AI 友好的 LSP 压缩代理 — 核心库。
//!
//! ## Feature Flags（功能特性）
//!
//! | Feature | 描述 | 默认值 |
//! |---------|-------------|---------|
//! | `cli` | CLI 二进制程序（clap, tracing-subscriber） | 是 |
//! | `mcp` | MCP 服务器（rmcp） | 否 |
//! | `agent-sdk` | Agent SDK API | 否 |
//! | `transport-tcp` | TCP 传输层 | 否 |
//! | `transport-websocket` | WebSocket 传输层 | 否 |
//!
//! ## Architecture（架构）
//!
//! [MermaidChart:docs/src/diagrams/architecture.mmd]

#![cfg_attr(docsrs, warn(missing_docs))]

pub mod codec;
pub mod config;
pub mod config_watcher;
pub mod error;
pub mod interceptors;
pub mod languages;
pub mod metrics;
pub mod proxy;
pub mod transport;

#[cfg(feature = "cli")]
pub mod init;

#[cfg(feature = "mcp")]
pub mod mcp;

#[cfg(feature = "agent-sdk")]
pub mod agent_sdk;

// Re-exports for convenience.
pub use config::{CappingConfig, Config, OutputFormat};
pub use error::LspzError;
pub use interceptors::capping::CappingInterceptor;
pub use interceptors::completions::CompletionCompressor;
pub use interceptors::diagnostics::DiagnosticsCompressor;
pub use interceptors::hover::HoverCompressor;
pub use interceptors::locations::LocationCompressor;
pub use interceptors::symbols::DocumentSymbolCompressor;
pub use interceptors::workspace_diagnostics::WorkspaceDiagnosticCompressor;
pub use interceptors::workspace_symbols::WorkspaceSymbolCompressor;
pub use metrics::MetricsConfig;
pub use proxy::Proxy;
pub use transport::Transport;
pub use transport::stdio::StdioTransport;
pub use transport::tcp::TcpTransport;

/// WebSocket 传输层（需要 `transport-websocket` 功能特性）。
#[cfg(feature = "transport-websocket")]
pub use transport::websocket::WsTransport;
