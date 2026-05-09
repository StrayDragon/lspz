//! # lspz – placeholder crate
//!
//! This crate is a **placeholder** for the upcoming `lspz` LSP proxy tool.
//! It currently does nothing; the real implementation is under development.
//!
//! ## Planned Features
//!
//! - Transparent LSP proxy between any Language Server and an AI Coding Agent.
//! - Token-aware compression of server-to-client messages (diagnostics, completions, etc.).
//! - Embeddable library (`lspz-core`) for integration into Rust-based agents.
//! - Standalone binary (`lspz`) for agents written in any language.
//!
//! ## When will it be ready?
//!
//! The first alpha release is expected soon. Check the repository for updates.
//!
//! ## Usage (placeholder)
//!
//! ```ignore
//! // This is a sketch of the future API.
//! use lspz::{Config, Proxy};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = Config {
//!         backend_cmd: vec!["typescript-language-server".into(), "--stdio".into()],
//!         enable_diag_compress: true,
//!         ..Default::default()
//!     };
//!     let proxy = Proxy::new(config).await.unwrap();
//!     proxy.serve().await.unwrap();
//! }
//! ```

/// Placeholder struct for configuration.
pub struct Config;

/// Placeholder struct for the proxy.
pub struct Proxy;

impl Proxy {
    /// Placeholder constructor.
    pub fn new(_config: Config) -> Result<Self, Box<dyn std::error::Error>> {
        unimplemented!("lspz is not yet implemented. This is a placeholder crate.")
    }
}
