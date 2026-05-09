//! CLI entry point for lspz — LSP compression proxy.
//!
//! # Usage
//!
//! ```bash
//! lspz --backend rust-analyzer
//! ```

use std::process::ExitCode;

use clap::Parser;
use lspz_core::interceptors::Interceptor;
use lspz_core::interceptors::InterceptorChain;
use lspz_core::interceptors::diagnostics::DiagnosticsCompressor;
use lspz_core::{Config, Proxy, StdioTransport};
use tracing_subscriber::EnvFilter;

/// AI-friendly LSP compression proxy.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Backend LSP server command (e.g. "rust-analyzer", "gopls")
    #[arg(short, long, env = "LSPZ_BACKEND_CMD")]
    backend: String,

    /// Enable diagnostic compression (default: true)
    #[arg(short, long, env = "LSPZ_ENABLE_DIAG_COMPRESS", default_value_t = true)]
    compress: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, env = "LSPZ_LOG_LEVEL", default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::builder().parse_lossy(&args.log_level))
        .with_target(false)
        .init();

    // Build config
    let config = match Config::builder()
        .backend_cmd(&args.backend)
        .enable_diag_compress(args.compress)
        .log_level(&args.log_level)
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
