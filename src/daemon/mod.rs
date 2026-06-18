//! # lspz Daemon
//!
//! Long-lived background process that manages LSP server sessions.
//!
//! ## Architecture
//!
//! The daemon listens on a Unix domain socket at `~/.lspz/<workspace-hash>.sock`.
//! Clients (MCP server, CLI) connect to it transparently. When no daemon is
//! running, the client auto-spawns one in the background.
//!
//! ## Protocol
//!
//! JSON-RPC 2.0 over newline-delimited JSON on Unix socket.
//!
//! ### Methods
//!
//! | Method | Description |
//! |--------|-------------|
//! | `lsp/spawn` | Get or create an LSP session |
//! | `lsp/request` | Send an LSP request, await response |
//! | `lsp/notify` | Send an LSP notification |
//! | `lsp/wait_notify` | Wait for a notification from the server |
//! | `daemon/status` | Query daemon state (sessions, PIDs, stats) |
//! | `daemon/shutdown` | Graceful shutdown |

mod client;
pub mod protocol;
mod server;
mod socket;
mod status;

pub use client::DaemonClient;
pub use protocol::{DaemonRequest, DaemonResponse, SpawnParams};
pub use server::DaemonServer;
pub use socket::socket_path_for_workspace;
pub use status::DaemonStatus;
