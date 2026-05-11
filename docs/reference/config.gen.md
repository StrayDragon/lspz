# 配置参考

> 自动从 `src/config.rs` 生成。编辑源码后运行 `just gen-config-docs` 刷新。

## `Config`

- **`backend_cmd`**: `String` — Command used to launch the backend LSP server.
- **`enable_diag_compress`**: `bool` — Whether to enable diagnostic compression.
- **`enable_completion_compress`**: `bool` — Whether to enable completion compression (default: true).
- **`enable_hover_compress`**: `bool` — Whether to enable hover compression (default: true).
- **`log_level`**: `String` — Log level (trace, debug, info, warn, error).

## `ConfigBuilder`
