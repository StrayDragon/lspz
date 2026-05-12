//! # lspz
//!
//! AI-friendly LSP compression proxy — core library.
//!
//! ## Feature Flags
//!
//! | Feature | Description | Default |
//! |---------|-------------|---------|
//! | `cli` | CLI binary (clap, tracing-subscriber) | yes |
//! | `mcp` | MCP server (rmcp) | no |
//! | `agent-sdk` | Agent SDK API | no |
//! | `transport-tcp` | TCP transport | no (always on) |
//! | `transport-websocket` | WebSocket transport | no |
//!
//! ## Architecture
//!
//! [MermaidChart:./docs/mmd/architecture.mmd]

pub mod codec;
pub mod config;
pub mod config_watcher;
pub mod error;
pub mod interceptors;
pub mod metrics;
pub mod proxy;
pub mod transport;

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

/// WebSocket transport (requires `transport-websocket` feature).
#[cfg(feature = "transport-websocket")]
pub use transport::websocket::WsTransport;
