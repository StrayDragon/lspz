//! # lspz MCP server
//!
//! Exposes LSP diagnostics, completions, and symbols as MCP tools
//! using the official [`rmcp`] SDK (2.x).
//!
//! ## Workspace resolution
//!
//! Coding agents should call `set_workspace` (or pass `workspace`) then use
//! project-relative `path` / `paths`. Absolute `file://` URIs remain supported.
//! MCP Roots are best-effort; untrusted cwd fallbacks such as `$HOME` are
//! rejected for scans instead of silently reading unrelated files.

mod daemon_server;
mod pool;
mod server;
mod session;
mod workspace;

pub use daemon_server::DaemonMcpServer;
pub use pool::{LspPool, pool_key};
pub use server::McpServer;
pub use session::{InitializeParams, LspSession};
