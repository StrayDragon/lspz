# API: `crates/lspz-core/src/config`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Configuration for the lspz proxy.

```rust
#[derive(Debug, Clone)]
```

Command used to launch the backend LSP server.

```rust
pub backend_cmd: String,
```

Whether to enable diagnostic compression.

```rust
pub enable_diag_compress: bool,
```

Whether to enable completion compression (default: true).

```rust
pub enable_completion_compress: bool,
```

Whether to enable hover compression (default: true).

```rust
pub enable_hover_compress: bool,
```

Whether to enable document symbol compression (default: true).

```rust
pub enable_document_symbol_compress: bool,
```

Log level (trace, debug, info, warn, error).

```rust
pub log_level: String,
```

Create a new [`ConfigBuilder`].

```rust
pub fn builder() -> ConfigBuilder {
```

Builder for [`Config`].

```rust
#[derive(Debug, Default)]
```

Set the backend LSP server command.

```rust
pub fn backend_cmd(mut self, cmd: impl Into<String>) -> Self {
```

Enable or disable diagnostic compression.

```rust
pub fn enable_diag_compress(mut self, enable: bool) -> Self {
```

Enable or disable completion compression.

```rust
pub fn enable_completion_compress(mut self, enable: bool) -> Self {
```

Enable or disable hover compression.

```rust
pub fn enable_hover_compress(mut self, enable: bool) -> Self {
```

Enable or disable document symbol compression.

```rust
pub fn enable_document_symbol_compress(mut self, enable: bool) -> Self {
```

Set the log level.

```rust
pub fn log_level(mut self, level: impl Into<String>) -> Self {
```

Build the [`Config`], validating required fields.

```rust
pub fn build(self) -> Result<Config, LspzError> {
```
