# 配置参考

> 自动从 `src/config.rs` 生成。编辑源码后运行 `just gen-config-docs` 刷新。

## `CappingConfig`

- **`max_diags`**: `usize` — Maximum number of diagnostics to keep (0 = unlimited).
- **`max_completions`**: `usize` — Maximum number of completion items to keep (0 = unlimited).
- **`max_symbols`**: `usize` — Maximum number of document symbols to keep (0 = unlimited).

## `Config`

- **`backend_cmd`**: `String` — Command used to launch the backend LSP server.
- **`capping`**: `CappingConfig`
- **`enable_diag_compress`**: `bool`
- **`enable_completion_compress`**: `bool`
- **`enable_hover_compress`**: `bool`
- **`enable_document_symbol_compress`**: `bool`
- **`enable_location_compress`**: `bool`
- **`enable_workspace_symbol_compress`**: `bool`
- **`enable_workspace_diag_compress`**: `bool`
- **`output_format`**: `OutputFormat`
- **`log_level`**: `String`
- **`metrics`**: `MetricsConfig`

## `ConfigBuilder`
