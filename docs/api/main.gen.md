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

Arguments to pass through to the backend LSP server
(placed after `--` on the command line)

```rust
#[arg(trailing_var_arg = true, allow_hyphen_values = true)]
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
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
```

Run as MCP server — exposes LSP tools via Model Context Protocol.

```rust
async fn run_mcp(log_level: String) -> ExitCode {
```
