# API: `crates/lspz/src/main`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Run in proxy mode — transparent LSP proxy with diagnostic compression

```rust
#[command(name = "proxy", alias = "p")]
```

Backend LSP server command (e.g. "rust-analyzer", "gopls")

```rust
#[arg(short, long, env = "LSPZ_BACKEND_CMD")]
```

Enable diagnostic compression (default: true)

```rust
#[arg(short, long, env = "LSPZ_ENABLE_DIAG_COMPRESS", default_value_t = true)]
```

Log level (trace, debug, info, warn, error)

```rust
#[arg(short, long, env = "LSPZ_LOG_LEVEL", default_value = "info")]
```

Run as MCP server — exposes LSP tools via Model Context Protocol

```rust
#[command(name = "mcp")]
```

Log level (trace, debug, info, warn, error)

```rust
#[arg(short, long, env = "LSPZ_LOG_LEVEL", default_value = "info")]
```

Run in proxy mode — transparent LSP proxy with diagnostic compression.

```rust
async fn run_proxy(backend: String, compress: bool, log_level: String) -> ExitCode {
```

Run as MCP server — exposes LSP tools via Model Context Protocol.

```rust
async fn run_mcp(log_level: String) -> ExitCode {
```
