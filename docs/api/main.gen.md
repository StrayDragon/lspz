# API: `src/main`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Grouped CLI arguments for proxy mode to reduce function parameter count.

```rust
struct ProxyArgs {
```

Run in proxy mode — transparent LSP proxy with diagnostic compression

```rust
#[command(name = "proxy", alias = "p")]
```

Backend LSP server command (e.g. "rust-analyzer", "gopls")

```rust
#[arg(short, long, env = "LSPZ_BACKEND_CMD")]
```

Arguments to pass through to the backend LSP server

```rust
#[arg(trailing_var_arg = true, allow_hyphen_values = true)]
```

Transport type: stdio, tcp://host:port, ws://url, wss://url

```rust
#[arg(
```

Enable diagnostic compression (default: true)

```rust
#[arg(
```

Enable completion compression (default: true)

```rust
#[arg(
```

Enable hover compression (default: true)

```rust
#[arg(
```

Enable document symbol compression (default: true)

```rust
#[arg(
```

Enable location compression (default: true)

```rust
#[arg(
```

Enable workspace symbol compression (default: true)

```rust
#[arg(
```

Enable workspace diagnostic compression (default: true)

```rust
#[arg(
```

Output format: toon, json (compact), or passthrough

```rust
#[arg(
```

Log level (trace, debug, info, warn, error)

```rust
#[arg(short, long, env = "LSPZ_LOG_LEVEL", default_value = "info")]
```

Maximum number of diagnostics to keep (0 = unlimited)

```rust
#[arg(long, env = "LSPZ_MAX_DIAGS", default_value_t = 0)]
```

Maximum number of completion items to keep (0 = unlimited)

```rust
#[arg(long, env = "LSPZ_MAX_COMPLETIONS", default_value_t = 0)]
```

Maximum number of document symbols to keep (0 = unlimited)

```rust
#[arg(long, env = "LSPZ_MAX_SYMBOLS", default_value_t = 0)]
```

Enable runtime metrics collection

```rust
#[arg(long, env = "LSPZ_METRICS_ENABLED", default_value_t = false)]
```

Metrics report interval in seconds (0 = only on shutdown)

```rust
#[arg(long, env = "LSPZ_METRICS_INTERVAL", default_value_t = 0)]
```

Path to TOML config file for hot-reload support

```rust
#[arg(long, env = "LSPZ_CONFIG_FILE")]
```

Run as MCP server — exposes LSP tools via Model Context Protocol

```rust
#[cfg(feature = "mcp")]
```

Log level (trace, debug, info, warn, error)

```rust
#[arg(short, long, env = "LSPZ_LOG_LEVEL", default_value = "info")]
```
