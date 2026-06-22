//! # lspz Daemon
//!
//! Long-lived background process that manages LSP server sessions.
//!
//! ## Architecture
//!
//! The daemon listens on a Unix domain socket at
//! `~/.cache/lspz/<slug>-<hash>.sock`, where `<hash>` is derived from the
//! **canonicalized** workspace root (see [`socket_path_for_workspace`] /
//! [`resolve_workspace_root`]). Canonicalization is what makes the daemon
//! reusable: the same project reached via different path spellings lands on
//! the same socket.
//!
//! Clients (MCP server, CLI) connect to it transparently. When no daemon is
//! running, the client auto-spawns one in the background. A background reaper
//! reclaims idle LSP sessions and, once the daemon has had no sessions and no
//! connections for a while, shuts the daemon down so detached processes
//! (spawned via `setsid()`) do not pile up.
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
pub use socket::{resolve_workspace_root, socket_path_for_workspace};
pub use status::DaemonStatus;
