# API: `lspz/src/main`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

AI-friendly LSP compression proxy.

```rust
#[derive(Parser, Debug)]
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
