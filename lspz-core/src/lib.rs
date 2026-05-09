//! # lspz-core
//!
//! Core library for lspz — an AI-friendly LSP compression proxy.
//!
//! ## Architecture
//!
//! lspz-core provides the foundational abstractions for building an LSP proxy:
//!
//! - **codec**: JSON-RPC 2.0 message framing ([`codec::json_rpc`]) and compact format ([`codec::compact`])
//! - **transport**: I/O abstraction ([`transport::Transport`]) with stdio implementation ([`transport::StdioTransport`])
//! - **error**: Unified error type ([`error::LspzError`])
//! - **config**: Runtime configuration ([`config::Config`])
//! - **proxy**: LSP proxy state machine ([`proxy::Proxy`])
//! - **interceptors**: Message transformation pipeline ([`interceptors::Interceptor`])
//!
//! [MermaidChart:./docs/mmd/architecture.mmd]

pub mod codec;
pub mod config;
pub mod error;
pub mod interceptors;
pub mod proxy;
pub mod transport;

// Re-exports for convenience.
pub use config::Config;
pub use error::LspzError;
pub use proxy::Proxy;
pub use transport::Transport;
pub use transport::stdio::StdioTransport;
