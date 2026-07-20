//! # lspz MCP server
//!
//! Exposes LSP diagnostics, completions, and symbols as MCP tools
//! using the official [`rmcp`] SDK (2.x).
//!
//! ## Workspace resolution
//!
//! Coding agents (Cursor, etc.) may omit `uri` on `get_diagnostics` /
//! `get_symbols`. The server then resolves the workspace via MCP Roots
//! (when advertised), falling back to process cwd, scans a capped set of
//! source files, and returns a dense TOON overview.

mod daemon_server;
mod pool;
mod server;
mod session;
mod workspace;

pub use daemon_server::DaemonMcpServer;
pub use pool::{LspPool, pool_key};
pub use server::McpServer;
pub use session::{InitializeParams, LspSession};
