# 配置参考

> 自动从 `src/config.rs` 生成。编辑源码后运行 `just gen-config-docs` 刷新。

## `CappingConfig`

- **`max_diags`**: `usize` — Maximum number of diagnostics to keep (0 = unlimited).
- **`max_completions`**: `usize` — Maximum number of completion items to keep (0 = unlimited).
- **`max_symbols`**: `usize` — Maximum number of document symbols to keep (0 = unlimited).

## `Config`

- **`backend_cmd`**: `String` — Command used to launch the backend LSP server.
- **`capping`**: `CappingConfig` — Per-type response capping limits.
- **`enable_diag_compress`**: `bool` — Whether to enable diagnostic compression.
- **`enable_completion_compress`**: `bool` — Whether to enable completion compression (default: true).
- **`enable_hover_compress`**: `bool` — Whether to enable hover compression (default: true).
- **`enable_document_symbol_compress`**: `bool` — Whether to enable document symbol compression (default: true).
- **`enable_location_compress`**: `bool` — Whether to enable location compression (default: true).
- **`enable_workspace_symbol_compress`**: `bool` — Whether to enable workspace symbol compression (default: true).
- **`enable_workspace_diag_compress`**: `bool` — Whether to enable workspace diagnostic compression (default: true).
- **`output_format`**: `OutputFormat` — Output format for intercepted messages (json, toon, passthrough).
- **`log_level`**: `String` — Log level (trace, debug, info, warn, error).

## `ConfigBuilder`
