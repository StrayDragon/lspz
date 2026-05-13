//! # lspz Agent SDK
//!
//! High-level API for embedding LSP capabilities into AI coding agents.

mod agent;
mod pool;

pub use agent::{AgentBuilder, AgentHandle};
pub use pool::{AgentPool, AgentPoolBuilder};
