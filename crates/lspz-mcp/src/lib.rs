//! # lspz-mcp — MCP server for lspz
//!
//! Exposes LSP diagnostics, completions, and symbols as MCP tools
//! using the official [`rmcp`] SDK.
//!
//! ## Quick Start
//!
//! ```ignore
//! use lspz_mcp::McpServer;
//! use rmcp::ServiceExt;
//! use rmcp::transport::stdio;
//!
//! let service = McpServer::new().serve(stdio()).await.unwrap();
//! service.waiting().await.unwrap();
//! ```

mod pool;
mod server;
mod session;

pub use pool::LspPool;
pub use server::McpServer;
pub use session::LspSession;
