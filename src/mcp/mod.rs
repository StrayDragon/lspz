//! # lspz MCP server
//!
//! Exposes LSP diagnostics, completions, and symbols as MCP tools
//! using the official [`rmcp`] SDK.

mod daemon_server;
mod pool;
mod server;
mod session;

pub use daemon_server::DaemonMcpServer;
pub use pool::LspPool;
pub use server::McpServer;
pub use session::{InitializeParams, LspSession};
