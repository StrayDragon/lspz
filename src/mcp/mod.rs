//! # lspz MCP server
//!
//! Exposes LSP diagnostics, completions, and symbols as MCP tools
//! using the official [`rmcp`] SDK.

mod pool;
mod server;
mod session;

pub use pool::LspPool;
pub use server::McpServer;
pub use session::LspSession;
