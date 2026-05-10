//! CLI entry point for lspz — LSP compression proxy and MCP server.
//!
//! # Usage
//!
//! ```bash
//! lspz --backend rust-analyzer       # Proxy mode (default)
//! lspz mcp                           # MCP server mode
//! ```

use std::process::ExitCode;

use clap::Parser;
use lspz_core::interceptors::Interceptor;
use lspz_core::interceptors::InterceptorChain;
use lspz_core::interceptors::diagnostics::DiagnosticsCompressor;
use lspz_core::{Config, Proxy, StdioTransport};
use rmcp::ServiceExt;
use rmcp::transport::stdio;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about)]
enum Cli {
    /// Run in proxy mode — transparent LSP proxy with diagnostic compression
    #[command(name = "proxy", alias = "p")]
    Proxy {
        /// Backend LSP server command (e.g. "rust-analyzer", "gopls")
        #[arg(short, long, env = "LSPZ_BACKEND_CMD")]
        backend: String,

        /// Enable diagnostic compression (default: true)
        #[arg(short, long, env = "LSPZ_ENABLE_DIAG_COMPRESS", default_value_t = true)]
        compress: bool,

        /// Log level (trace, debug, info, warn, error)
        #[arg(short, long, env = "LSPZ_LOG_LEVEL", default_value = "info")]
        log_level: String,
    },

    /// Run as MCP server — exposes LSP tools via Model Context Protocol
    #[command(name = "mcp")]
    Mcp {
        /// Log level (trace, debug, info, warn, error)
        #[arg(short, long, env = "LSPZ_LOG_LEVEL", default_value = "info")]
        log_level: String,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    match Cli::parse() {
        Cli::Proxy {
            backend,
            compress,
            log_level,
        } => run_proxy(backend, compress, log_level).await,
        Cli::Mcp { log_level } => run_mcp(log_level).await,
    }
}

/// Run in proxy mode — transparent LSP proxy with diagnostic compression.
async fn run_proxy(backend: String, compress: bool, log_level: String) -> ExitCode {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::builder().parse_lossy(&log_level))
        .with_target(false)
        .init();

    // Build config
    let config = match Config::builder()
        .backend_cmd(&backend)
        .enable_diag_compress(compress)
        .log_level(&log_level)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Create transport
    let transport = match StdioTransport::spawn(&config.backend_cmd) {
        Ok(t) => Box::new(t) as Box<dyn lspz_core::Transport>,
        Err(e) => {
            eprintln!("Failed to start backend server: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Build interceptor chain
    let mut interceptors: Vec<Box<dyn Interceptor>> = Vec::new();
    if config.enable_diag_compress {
        interceptors.push(Box::new(DiagnosticsCompressor::default()));
        tracing::info!("Diagnostic compression enabled");
    }
    let interceptor_chain = InterceptorChain::new(interceptors);

    // Create and start proxy
    let mut proxy = Proxy::new(config, transport, interceptor_chain);

    if let Err(e) = proxy.start().await {
        tracing::error!(error = %e, "Proxy exited with error");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

/// Run as MCP server — exposes LSP tools via Model Context Protocol.
async fn run_mcp(log_level: String) -> ExitCode {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::builder().parse_lossy(&log_level))
        .with_target(false)
        .init();

    tracing::info!("Starting lspz MCP server");

    let server = lspz_mcp::McpServer::new();

    match server.serve(stdio()).await {
        Ok(service) => {
            tracing::info!("MCP server ready (stdio)");
            if let Err(e) = service.waiting().await {
                tracing::error!(error = %e, "MCP server exited with error");
                return ExitCode::FAILURE;
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to start MCP server");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}
