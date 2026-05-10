//! # lspz-agent-sdk — Agent SDK for lspz
//!
//! High-level API for embedding LSP capabilities into AI coding agents.
//!
//! ## Quick Start
//!
//! ```ignore
//! use lspz_agent_sdk::AgentHandle;
//!
//! let mut agent = AgentHandle::builder()
//!     .backend("rust-analyzer")
//!     .language("rust")
//!     .start()
//!     .await?;
//!
//! let diagnostics = agent.get_diagnostics("file:///path/to/file.rs").await?;
//! agent.shutdown().await?;
//! ```

mod agent;

pub use agent::AgentHandle;
