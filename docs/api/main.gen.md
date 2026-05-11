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
#[allow(clippy::too_many_arguments)]
```

Run as MCP server — exposes LSP tools via Model Context Protocol.

```rust
async fn run_mcp(log_level: String) -> ExitCode {
```
