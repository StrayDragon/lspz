//! CLI entry point for lspz — LSP compression proxy and MCP server.
//!
//! ```bash
//! lspz --backend rust-analyzer       # Proxy mode (default)
//! lspz mcp                           # MCP server mode
//! ```

use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use lspz::Transport;
use lspz::config_watcher::ConfigWatcher;
use lspz::interceptors::Interceptor;
use lspz::interceptors::InterceptorChain;
use lspz::interceptors::capping::CappingInterceptor;
use lspz::interceptors::default_interceptors;
use lspz::metrics::MetredInterceptor;
use lspz::{
    CappingConfig, Config, MetricsConfig, OutputFormat, Proxy, StdioTransport, TcpTransport,
};
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

#[cfg(feature = "mcp")]
use rmcp::ServiceExt;
#[cfg(feature = "mcp")]
use rmcp::transport::stdio;

#[cfg(feature = "transport-websocket")]
use lspz::WsTransport;

/// Grouped CLI arguments for proxy mode to reduce function parameter count.
struct ProxyArgs {
    backend: String,
    backend_args: Vec<String>,
    transport_scheme: String,
    output: String,
    log_level: String,
    max_diags: usize,
    max_completions: usize,
    max_symbols: usize,
    metrics_enabled: bool,
    metrics_interval: u64,
    config_file: Option<String>,
    compress_diag: bool,
    compress_completion: bool,
    compress_hover: bool,
    compress_document_symbol: bool,
    compress_location: bool,
    compress_workspace_symbol: bool,
    compress_workspace_diag: bool,
}

#[derive(Parser, Debug)]
#[command(version, about)]
enum Cli {
    /// Run daemon — long-lived background LSP session manager
    #[command(name = "daemon", alias = "d")]
    Daemon {
        /// Path to the Unix domain socket
        #[arg(long)]
        socket: Option<String>,

        /// Log level (trace, debug, info, warn, error)
        #[arg(short, long, env = "LSPZ_LOG_LEVEL", default_value = "info")]
        log_level: String,

        #[command(subcommand)]
        command: Option<DaemonCommand>,
    },

    /// Run in proxy mode — transparent LSP proxy with diagnostic compression
    #[command(name = "proxy", alias = "p")]
    Proxy {
        /// Backend LSP server command (e.g. "rust-analyzer", "gopls")
        #[arg(short, long, env = "LSPZ_BACKEND_CMD")]
        backend: String,

        /// Arguments to pass through to the backend LSP server
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        backend_args: Vec<String>,

        /// Transport type: stdio, tcp://host:port, ws://url, wss://url
        #[arg(
            short = 't',
            long = "transport",
            env = "LSPZ_TRANSPORT",
            default_value = "stdio"
        )]
        transport: String,

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

        /// Enable runtime metrics collection
        #[arg(long, env = "LSPZ_METRICS_ENABLED", default_value_t = false)]
        metrics: bool,

        /// Metrics report interval in seconds (0 = only on shutdown)
        #[arg(long, env = "LSPZ_METRICS_INTERVAL", default_value_t = 0)]
        metrics_interval: u64,

        /// Path to TOML config file for hot-reload support
        #[arg(long, env = "LSPZ_CONFIG_FILE")]
        config: Option<String>,
    },

    /// Run as MCP server — exposes LSP tools via Model Context Protocol
    #[command(name = "mcp")]
    Mcp {
        /// Log level (trace, debug, info, warn, error)
        #[arg(short, long, env = "LSPZ_LOG_LEVEL", default_value = "info")]
        log_level: String,

        /// Disable daemon auto-connect (use in-process LSP pool instead)
        #[arg(long)]
        no_daemon: bool,
    },

    /// Initialize Claude Code integration (MCP server registration, context injection)
    #[command(name = "init")]
    Init {
        /// Add to global config (~/.claude/settings.json) instead of project-local
        #[arg(short, long)]
        global: bool,

        /// Auto-patch settings.json without prompting
        #[arg(long = "auto-patch")]
        auto_patch: bool,

        /// Skip settings.json patching (print manual instructions)
        #[arg(long = "no-patch")]
        no_patch: bool,

        /// Show current lspz configuration
        #[arg(long)]
        show: bool,

        /// Remove lspz artifacts from Claude Code settings
        #[arg(long)]
        uninstall: bool,

        /// Preview changes without writing any files
        #[arg(long = "dry-run")]
        dry_run: bool,

        /// Force overwrite even if files are already up to date
        #[arg(short, long)]
        force: bool,
    },
}

/// Subcommands for `lspz daemon`.
#[derive(clap::Subcommand, Debug)]
enum DaemonCommand {
    /// List active LSP server sessions managed by the daemon
    List {
        /// Workspace root path (auto-detected from current directory if omitted)
        #[arg(short, long)]
        workspace: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Output as TOON (token-optimized tabular)
        #[arg(long)]
        toon: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    match Cli::parse() {
        Cli::Daemon {
            socket,
            log_level,
            command,
        } => match command {
            Some(DaemonCommand::List {
                workspace,
                json,
                toon,
            }) => run_daemon_list(workspace, json, toon).await,
            None => run_daemon(socket, log_level).await,
        },
        Cli::Proxy {
            backend,
            backend_args,
            transport,
            output,
            log_level,
            max_diags,
            max_completions,
            max_symbols,
            metrics,
            metrics_interval,
            config,
            compress_diag,
            compress_completion,
            compress_hover,
            compress_document_symbol,
            compress_location,
            compress_workspace_symbol,
            compress_workspace_diag,
        } => {
            let args = ProxyArgs {
                backend,
                backend_args,
                transport_scheme: transport,
                output,
                log_level,
                max_diags,
                max_completions,
                max_symbols,
                metrics_enabled: metrics,
                metrics_interval,
                config_file: config,
                compress_diag,
                compress_completion,
                compress_hover,
                compress_document_symbol,
                compress_location,
                compress_workspace_symbol,
                compress_workspace_diag,
            };
            run_proxy(args).await
        }
        #[cfg(feature = "mcp")]
        Cli::Mcp {
            log_level,
            no_daemon,
        } => run_mcp(log_level, no_daemon).await,
        #[cfg(not(feature = "mcp"))]
        Cli::Mcp { .. } => {
            eprintln!(
                "Error: MCP server is not enabled in this build.\n  \
                 Rebuild with `--features mcp` to enable MCP support."
            );
            ExitCode::FAILURE
        }
        Cli::Init {
            global,
            auto_patch,
            no_patch,
            show,
            uninstall,
            dry_run,
            force,
        } => lspz::init::run(
            global, auto_patch, no_patch, show, uninstall, dry_run, force,
        ),
    }
}

async fn run_proxy(args: ProxyArgs) -> ExitCode {
    if let Some(code) = handle_backend_help(&args.backend, &args.backend_args).await {
        return code;
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::builder().parse_lossy(&args.log_level))
        .with_target(false)
        .init();

    let output_format = match OutputFormat::from_str(&args.output) {
        Ok(fmt) => fmt,
        Err(_) => {
            eprintln!(
                "Unknown output format '{}'. Use 'toon', 'json', or 'passthrough'.",
                args.output
            );
            return ExitCode::FAILURE;
        }
    };

    let base_config = match build_config(&args, output_format) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let config = match &args.config_file {
        Some(path) => match Config::from_file(path) {
            Ok(file_config) => {
                tracing::info!(path = %path, "Loaded config file");
                Config {
                    backend_cmd: base_config.backend_cmd.clone(),
                    ..file_config
                }
            }
            Err(e) => {
                eprintln!("Failed to load config file '{path}': {e}");
                return ExitCode::FAILURE;
            }
        },
        None => base_config,
    };

    let shared_config = Arc::new(RwLock::new(config));

    let backend_cmd = shared_config.read().await.backend_cmd.clone();
    let transport: Box<dyn Transport> =
        match create_transport(&args.transport_scheme, &backend_cmd, &args.backend_args).await {
            Ok(t) => t,
            Err(code) => return code,
        };

    let interceptor_chain = build_interceptor_chain(&shared_config);

    let mut proxy = Proxy::new(shared_config, transport, interceptor_chain);

    if let Some(ref path) = args.config_file {
        match ConfigWatcher::spawn(path, proxy.shared_config()) {
            Ok(watcher) => {
                proxy.set_config_watcher(watcher);
                tracing::info!(path = %path, "Config hot-reload enabled");
            }
            Err(e) => {
                tracing::warn!(error = %e, "Config watcher failed, running without hot-reload");
            }
        }
    }

    if let Err(e) = proxy.start().await {
        tracing::error!(error = %e, "Proxy exited with error");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

async fn handle_backend_help(backend: &str, backend_args: &[String]) -> Option<ExitCode> {
    if !backend_args.iter().any(|a| a == "--help" || a == "-h") {
        return None;
    }

    let parts = shell_words::split(backend).ok()?;
    let mut iter = parts.into_iter();
    let program = iter.next()?;
    let mut args: Vec<String> = iter.collect();
    args.extend(backend_args.iter().cloned());

    let status = tokio::process::Command::new(&program)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .await;

    match status {
        Ok(s) if s.success() => Some(ExitCode::SUCCESS),
        _ => Some(ExitCode::FAILURE),
    }
}

async fn create_transport(
    scheme: &str,
    backend_cmd: &str,
    backend_args: &[String],
) -> Result<Box<dyn Transport>, ExitCode> {
    if scheme == "stdio" {
        StdioTransport::spawn(backend_cmd, backend_args)
            .map(|t| Box::new(t) as Box<dyn Transport>)
            .map_err(|e| {
                eprintln!("Failed to start backend server: {e}");
                ExitCode::FAILURE
            })
    } else if let Some(addr) = scheme.strip_prefix("tcp://") {
        TcpTransport::connect(addr)
            .await
            .map(|t| {
                tracing::info!(addr = %addr, "Connected to TCP LSP server");
                Box::new(t) as Box<dyn Transport>
            })
            .map_err(|e| {
                eprintln!("Failed to connect to TCP server '{addr}': {e}");
                ExitCode::FAILURE
            })
    } else if scheme.starts_with("ws://") || scheme.starts_with("wss://") {
        connect_websocket(scheme).await
    } else {
        eprintln!(
            "Unknown transport scheme '{scheme}'. Use 'stdio', 'tcp://host:port', or 'ws://url'."
        );
        Err(ExitCode::FAILURE)
    }
}

async fn connect_websocket(scheme: &str) -> Result<Box<dyn Transport>, ExitCode> {
    #[cfg(feature = "transport-websocket")]
    {
        WsTransport::connect(scheme)
            .await
            .map(|t| {
                tracing::info!(url = %scheme, "Connected to WebSocket LSP server");
                Box::new(t) as Box<dyn Transport>
            })
            .map_err(|e| {
                eprintln!("Failed to connect to WebSocket server '{scheme}': {e}");
                ExitCode::FAILURE
            })
    }
    #[cfg(not(feature = "transport-websocket"))]
    {
        let _ = scheme;
        eprintln!("WebSocket transport is not enabled. Build with `transport-websocket` feature.");
        Err(ExitCode::FAILURE)
    }
}

fn build_config(args: &ProxyArgs, output_format: OutputFormat) -> Result<Config, ExitCode> {
    Config::builder()
        .backend_cmd(&args.backend)
        .capping(CappingConfig {
            max_diags: args.max_diags,
            max_completions: args.max_completions,
            max_symbols: args.max_symbols,
        })
        .enable_diag_compress(args.compress_diag)
        .enable_completion_compress(args.compress_completion)
        .enable_hover_compress(args.compress_hover)
        .enable_document_symbol_compress(args.compress_document_symbol)
        .enable_location_compress(args.compress_location)
        .enable_workspace_symbol_compress(args.compress_workspace_symbol)
        .enable_workspace_diag_compress(args.compress_workspace_diag)
        .output_format(output_format)
        .log_level(&args.log_level)
        .metrics(MetricsConfig {
            enabled: args.metrics_enabled,
            report_interval_secs: args.metrics_interval,
        })
        .build()
        .map_err(|e| {
            eprintln!("Configuration error: {e}");
            ExitCode::FAILURE
        })
}

fn build_interceptor_chain(shared_config: &Arc<RwLock<Config>>) -> InterceptorChain {
    let config = shared_config.blocking_read();

    let mut interceptors: Vec<Box<dyn Interceptor>> = vec![Box::new(CappingInterceptor::new(
        config.capping.max_diags,
        config.capping.max_completions,
        config.capping.max_symbols,
    ))];
    interceptors.extend(default_interceptors());

    tracing::info!(
        capping = %config.capping.any_enabled(),
        diagnostics = %config.enable_diag_compress,
        completions = %config.enable_completion_compress,
        hover = %config.enable_hover_compress,
        document_symbols = %config.enable_document_symbol_compress,
        locations = %config.enable_location_compress,
        workspace_symbols = %config.enable_workspace_symbol_compress,
        workspace_diags = %config.enable_workspace_diag_compress,
        "Interceptor chain built (all interceptors, gated by runtime config)"
    );

    let metrics_enabled = config.metrics.enabled;
    drop(config);

    if metrics_enabled {
        tracing::info!("Metrics collection enabled");
        let wrapped: Vec<Box<dyn Interceptor>> = interceptors
            .into_iter()
            .map(|i| Box::new(MetredInterceptor::new(i).enable()) as Box<dyn Interceptor>)
            .collect();
        InterceptorChain::new(wrapped, shared_config.clone())
    } else {
        InterceptorChain::new(interceptors, shared_config.clone())
    }
}

/// Start lspz daemon in the background.
#[cfg(not(feature = "mcp"))]
async fn run_daemon(_socket: Option<String>, _log_level: String) -> ExitCode {
    eprintln!(
        "Error: Daemon mode is not enabled in this build.\n  \
         Rebuild with `--features mcp` to enable daemon support."
    );
    ExitCode::FAILURE
}

/// Query daemon status (stub when mcp is disabled).
#[cfg(not(feature = "mcp"))]
async fn run_daemon_list(_workspace: Option<String>, _json: bool, _toon: bool) -> ExitCode {
    eprintln!(
        "Error: daemon list is not enabled in this build.\n  \
         Rebuild with `--features mcp` to enable daemon support."
    );
    ExitCode::FAILURE
}

/// Start lspz daemon in the background.
#[cfg(feature = "mcp")]
async fn run_daemon(socket: Option<String>, log_level: String) -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::builder().parse_lossy(&log_level))
        .with_target(false)
        .init();

    let workspace_root = std::env::var("LSPZ_DAEMON_WORKSPACE")
        .map(|v| resolve_workspace(&v))
        .unwrap_or_else(|_| {
            std::env::current_dir()
                .map(|p| resolve_workspace(&p.to_string_lossy()))
                .unwrap_or_else(|_| ".".into())
        });

    let socket_path = match socket {
        Some(p) => std::path::PathBuf::from(p),
        None => lspz::daemon::socket_path_for_workspace(&workspace_root),
    };

    use lspz::daemon::DaemonServer;
    let server = DaemonServer::new(socket_path);
    if let Err(e) = server.start().await {
        eprintln!("Daemon failed: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Query daemon status.
#[cfg(feature = "mcp")]
async fn run_daemon_list(workspace: Option<String>, json: bool, toon: bool) -> ExitCode {
    let workspace_root = match workspace {
        Some(w) => resolve_workspace(&w),
        None => std::env::current_dir()
            .map(|p| resolve_workspace(&p.to_string_lossy()))
            .unwrap_or_else(|_| ".".into()),
    };

    use lspz::daemon::DaemonClient;
    match DaemonClient::try_connect(&workspace_root).await {
        Ok(Some(mut client)) => {
            match client.get_status().await {
                Ok(status) => {
                    // Parse into DaemonStatus for structured access
                    let ds: lspz::daemon::DaemonStatus =
                        match serde_json::from_value(status.clone()) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("Error parsing daemon status: {e}");
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&status).unwrap_or_default()
                                );
                                return ExitCode::FAILURE;
                            }
                        };

                    if toon {
                        println!("{}", format_status_toon(&ds));
                    } else if json {
                        println!("{}", serde_json::to_string_pretty(&ds).unwrap_or_default());
                    } else {
                        println!("{}", format_status_human(&ds));
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Error querying daemon status: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Ok(None) => {
            println!("No lspz daemon running for workspace: {workspace_root}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Connection error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Format daemon status as human-friendly text.
#[cfg(feature = "mcp")]
fn format_status_human(ds: &lspz::daemon::DaemonStatus) -> String {
    let mut out = String::new();
    let uptime_m = ds.uptime_secs / 60;
    let uptime_s = ds.uptime_secs % 60;

    out.push_str(&format!("  Uptime: {uptime_m}m {uptime_s}s\n"));
    out.push_str(&format!("  Requests: {}\n", ds.total_requests));
    out.push_str(&format!("  Connections: {}\n", ds.total_connections));
    out.push_str(&format!("  Sessions: {}\n", ds.sessions.len()));

    if ds.sessions.is_empty() {
        out.push_str("\n  (no active sessions)\n");
    } else {
        for s in &ds.sessions {
            let age_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .saturating_sub(s.created_at);
            let idle_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .saturating_sub(s.last_used_at);
            let age_m = age_secs / 60;
            let idle_s = idle_secs % 60;

            out.push('\n');
            out.push_str(&format!("  {} ({})\n", s.language, s.backend));
            if let Some(root) = &s.workspace_root {
                out.push_str(&format!("    workspace: {root}\n"));
            }
            out.push_str(&format!("    key: {}\n", s.key));
            out.push_str(&format!("    requests: {}\n", s.request_count));
            out.push_str(&format!("    age: {age_m}m\n"));
            out.push_str(&format!("    idle: {idle_s}s\n"));
        }
    }
    out
}

/// Format daemon status as TOON text.
#[cfg(feature = "mcp")]
fn format_status_toon(ds: &lspz::daemon::DaemonStatus) -> String {
    let mut out = String::from("# daemon status\n");
    out.push_str(&format!("# uptime_secs={}\n", ds.uptime_secs));
    out.push_str(&format!("# total_requests={}\n", ds.total_requests));
    out.push_str(&format!("# total_connections={}\n", ds.total_connections));
    out.push_str(&format!("# sessions={}\n", ds.sessions.len()));

    if !ds.sessions.is_empty() {
        out.push_str("#\n");
        out.push_str("# key | language | backend | workspace | requests | age_s | idle_s\n");
        out.push_str("# --- | -------- | ------- | --------- | -------- | ----- | ------\n");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for s in &ds.sessions {
            let age = now.saturating_sub(s.created_at);
            let idle = now.saturating_sub(s.last_used_at);
            let root = s.workspace_root.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "{} | {} | {} | {} | {} | {} | {}\n",
                s.key, s.language, s.backend, root, s.request_count, age, idle
            ));
        }
    }
    out
}

/// Canonicalize a workspace root the same way the daemon socket layer does.
///
/// All daemon-aware CLI entry points (`mcp`, `daemon`, `daemon list`) route
/// their workspace string through here so that the same project reached via
/// different path spellings lands on the same daemon.
#[cfg(feature = "mcp")]
fn resolve_workspace(raw: &str) -> String {
    lspz::daemon::resolve_workspace_root(raw)
}

#[cfg(feature = "mcp")]
async fn run_mcp(log_level: String, no_daemon: bool) -> ExitCode {
    // Write tracing to a file to avoid polluting MCP's stdio JSON-RPC stream.
    let log_dir = dirs::cache_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    let log_path = log_dir.join("lspz-mcp.log");
    let log_file = std::fs::File::create(&log_path).unwrap_or_else(|e| {
        eprintln!("lspz: cannot create {}: {e}", log_path.display());
        std::process::exit(1)
    });
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::builder().parse_lossy(&log_level))
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(log_file))
        .init();

    if no_daemon {
        tracing::info!("Starting lspz MCP server (in-process)");
        let server = lspz::mcp::McpServer::new();
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
    } else {
        tracing::info!("Starting lspz MCP server (daemon mode)");
        let workspace = std::env::current_dir()
            .map(|p| resolve_workspace(&p.to_string_lossy()))
            .unwrap_or_else(|_| ".".into());
        let server = lspz::mcp::DaemonMcpServer::new(workspace);
        match server.serve(stdio()).await {
            Ok(service) => {
                tracing::info!("MCP server ready (stdio, daemon-backed)");
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
    }

    ExitCode::SUCCESS
}
