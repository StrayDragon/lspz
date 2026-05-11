//! CLI entry point for lspz — LSP compression proxy and MCP server.
//!
//! # Usage
//!
//! ```bash
//! lspz --backend rust-analyzer       # Proxy mode (default)
//! lspz mcp                           # MCP server mode
//! ```

use std::process::ExitCode;
use std::str::FromStr;

use clap::Parser;
use lspz_core::interceptors::Interceptor;
use lspz_core::interceptors::InterceptorChain;
use lspz_core::interceptors::capping::CappingInterceptor;
use lspz_core::interceptors::diagnostics::DiagnosticsCompressor;
use lspz_core::{CappingConfig, Config, OutputFormat, Proxy, StdioTransport};
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

        /// Arguments to pass through to the backend LSP server
        /// (placed after `--` on the command line)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        backend_args: Vec<String>,

        /// Enable diagnostic compression (default: true)
        #[arg(
            short = 'd',
            long = "compress-diag",
            env = "LSPZ_ENABLE_DIAG_COMPRESS",
            default_value_t = true
        )]
        compress_diag: bool,

        /// Enable completion compression (default: true)
        #[arg(
            short = 'c',
            long = "compress-completion",
            env = "LSPZ_ENABLE_COMPLETION_COMPRESS",
            default_value_t = true
        )]
        compress_completion: bool,

        /// Enable hover compression (default: true)
        #[arg(
            short = 'H',
            long = "compress-hover",
            env = "LSPZ_ENABLE_HOVER_COMPRESS",
            default_value_t = true
        )]
        compress_hover: bool,

        /// Enable document symbol compression (default: true)
        #[arg(
            short = 'S',
            long = "compress-document-symbol",
            env = "LSPZ_ENABLE_DOCUMENT_SYMBOL_COMPRESS",
            default_value_t = true
        )]
        compress_document_symbol: bool,

        /// Enable location compression (default: true)
        #[arg(
            short = 'L',
            long = "compress-location",
            env = "LSPZ_ENABLE_LOCATION_COMPRESS",
            default_value_t = true
        )]
        compress_location: bool,

        /// Enable workspace symbol compression (default: true)
        #[arg(
            short = 'W',
            long = "compress-workspace-symbol",
            env = "LSPZ_ENABLE_WORKSPACE_SYMBOL_COMPRESS",
            default_value_t = true
        )]
        compress_workspace_symbol: bool,

        /// Enable workspace diagnostic compression (default: true)
        #[arg(
            long = "compress-workspace-diag",
            env = "LSPZ_ENABLE_WORKSPACE_DIAG_COMPRESS",
            default_value_t = true
        )]
        compress_workspace_diag: bool,

        /// Output format: toon, json (compact), or passthrough
        #[arg(
            short = 'o',
            long = "output",
            env = "LSPZ_OUTPUT_FORMAT",
            default_value = "toon"
        )]
        output: String,

        /// Log level (trace, debug, info, warn, error)
        #[arg(short, long, env = "LSPZ_LOG_LEVEL", default_value = "info")]
        log_level: String,

        /// Maximum number of diagnostics to keep (0 = unlimited)
        #[arg(long, env = "LSPZ_MAX_DIAGS", default_value_t = 0)]
        max_diags: usize,

        /// Maximum number of completion items to keep (0 = unlimited)
        #[arg(long, env = "LSPZ_MAX_COMPLETIONS", default_value_t = 0)]
        max_completions: usize,

        /// Maximum number of document symbols to keep (0 = unlimited)
        #[arg(long, env = "LSPZ_MAX_SYMBOLS", default_value_t = 0)]
        max_symbols: usize,
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
            backend_args,
            compress_diag,
            compress_completion,
            compress_hover,
            compress_document_symbol,
            compress_location,
            compress_workspace_symbol,
            compress_workspace_diag,
            output,
            log_level,
            max_diags,
            max_completions,
            max_symbols,
        } => {
            run_proxy(
                backend,
                backend_args,
                compress_diag,
                compress_completion,
                compress_hover,
                compress_document_symbol,
                compress_location,
                compress_workspace_symbol,
                compress_workspace_diag,
                output,
                log_level,
                max_diags,
                max_completions,
                max_symbols,
            )
            .await
        }
        Cli::Mcp { log_level } => run_mcp(log_level).await,
    }
}

/// Run in proxy mode — transparent LSP proxy with diagnostic compression.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
async fn run_proxy(
    backend: String,
    backend_args: Vec<String>,
    compress_diag: bool,
    compress_completion: bool,
    compress_hover: bool,
    compress_document_symbol: bool,
    compress_location: bool,
    compress_workspace_symbol: bool,
    compress_workspace_diag: bool,
    output: String,
    log_level: String,
    max_diags: usize,
    max_completions: usize,
    max_symbols: usize,
) -> ExitCode {
    // If backend args contain --help or -h, spawn backend directly and show its help output
    if backend_args.iter().any(|a| a == "--help" || a == "-h") {
        let parts = match shell_words::split(&backend) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to parse backend command '{backend}': {e}");
                return ExitCode::FAILURE;
            }
        };
        let mut iter = parts.into_iter();
        let program = match iter.next() {
            Some(p) => p,
            None => {
                eprintln!("Empty backend command");
                return ExitCode::FAILURE;
            }
        };
        let mut args: Vec<String> = iter.collect();
        args.extend(backend_args);

        let status = match tokio::process::Command::new(&program)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await
        {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to execute backend '{program}': {e}");
                return ExitCode::FAILURE;
            }
        };

        return if status.success() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::builder().parse_lossy(&log_level))
        .with_target(false)
        .init();

    // Parse output format
    let output_format = OutputFormat::from_str(&output)
        .ok()
        .unwrap_or(OutputFormat::Json);

    // Build CappingConfig from CLI flags
    let capping_config = CappingConfig {
        max_diags,
        max_completions,
        max_symbols,
    };

    // Build config
    let config = match Config::builder()
        .backend_cmd(&backend)
        .capping(capping_config)
        .enable_diag_compress(compress_diag)
        .enable_completion_compress(compress_completion)
        .enable_hover_compress(compress_hover)
        .enable_document_symbol_compress(compress_document_symbol)
        .enable_location_compress(compress_location)
        .enable_workspace_symbol_compress(compress_workspace_symbol)
        .enable_workspace_diag_compress(compress_workspace_diag)
        .output_format(output_format)
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
    let transport = match StdioTransport::spawn(&config.backend_cmd, &backend_args) {
        Ok(t) => Box::new(t) as Box<dyn lspz_core::Transport>,
        Err(e) => {
            eprintln!("Failed to start backend server: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Build interceptor chain (capping first, then compressors)
    let mut interceptors: Vec<Box<dyn Interceptor>> = Vec::new();
    if config.capping.any_enabled() {
        interceptors.push(Box::new(CappingInterceptor::new(
            config.capping.max_diags,
            config.capping.max_completions,
            config.capping.max_symbols,
        )));
        tracing::info!(
            max_diags = config.capping.max_diags,
            max_completions = config.capping.max_completions,
            max_symbols = config.capping.max_symbols,
            "Response capping enabled"
        );
    }
    if config.enable_diag_compress {
        interceptors.push(Box::new(DiagnosticsCompressor::default()));
        tracing::info!("Diagnostic compression enabled");
    }
    if config.enable_completion_compress {
        interceptors.push(Box::new(lspz_core::CompletionCompressor::default()));
        tracing::info!("Completion compression enabled");
    }
    if config.enable_hover_compress {
        interceptors.push(Box::new(lspz_core::HoverCompressor::default()));
        tracing::info!("Hover compression enabled");
    }
    if config.enable_document_symbol_compress {
        interceptors.push(Box::new(lspz_core::DocumentSymbolCompressor));
        tracing::info!("Document symbol compression enabled");
    }
    if config.enable_location_compress {
        interceptors.push(Box::new(lspz_core::LocationCompressor));
        tracing::info!("Location compression enabled");
    }
    if config.enable_workspace_symbol_compress {
        interceptors.push(Box::new(lspz_core::WorkspaceSymbolCompressor));
        tracing::info!("Workspace symbol compression enabled");
    }
    if config.enable_workspace_diag_compress {
        interceptors.push(Box::new(lspz_core::WorkspaceDiagnosticCompressor));
        tracing::info!("Workspace diagnostic compression enabled");
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
