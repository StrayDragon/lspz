# API: `src/config`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Output format for the proxy.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
```

Compact JSON (current default).

```rust
Json,
```

TOON (Token-Oriented Object Notation).

```rust
Toon,
```

Standard LSP JSON passthrough (no compression in output).

```rust
Passthrough,
```

Per-type capping limits for LSP server responses.

A value of 0 means no limit (capping disabled for that type).

```rust
#[derive(Debug, Clone, Default, Deserialize)]
```

Maximum number of diagnostics to keep (0 = unlimited).

```rust
pub max_diags: usize,
```

Maximum number of completion items to keep (0 = unlimited).

```rust
pub max_completions: usize,
```

Maximum number of document symbols to keep (0 = unlimited).

```rust
pub max_symbols: usize,
```

Returns `true` if any capping limit is set.

```rust
pub fn any_enabled(&self) -> bool {
```

Configuration for the lspz proxy.

```rust
#[derive(Debug, Clone, Deserialize)]
```

Command used to launch the backend LSP server.

```rust
pub backend_cmd: String,
```

Per-type response capping limits.

```rust
#[serde(default)]
```

Whether to enable diagnostic compression.

```rust
#[serde(default = "default_true")]
```

Whether to enable completion compression (default: true).

```rust
#[serde(default = "default_true")]
```

Whether to enable hover compression (default: true).

```rust
#[serde(default = "default_true")]
```

Whether to enable document symbol compression (default: true).

```rust
#[serde(default = "default_true")]
```

Whether to enable location compression (default: true).

```rust
#[serde(default = "default_true")]
```

Whether to enable workspace symbol compression (default: true).

```rust
#[serde(default = "default_true")]
```

Whether to enable workspace diagnostic compression (default: true).

```rust
#[serde(default = "default_true")]
```

Output format for intercepted messages (json, toon, passthrough).

```rust
#[serde(default = "default_output_format")]
```

Log level (trace, debug, info, warn, error).

```rust
#[serde(default = "default_log_level")]
```

Runtime metrics configuration.

```rust
#[serde(default)]
```

Create a new [`ConfigBuilder`].

```rust
pub fn builder() -> ConfigBuilder {
```

Load config from a TOML file.

Missing fields use their default values (same as `Config::default()`).

```rust
pub fn from_file(path: impl AsRef<Path>) -> Result<Self, LspzError> {
```

Returns `true` if the named interceptor is enabled in this config.

Used by [`InterceptorChain`](crate::interceptors::InterceptorChain) at runtime
to skip disabled interceptors without removing them from the chain.

```rust
pub fn is_interceptor_enabled(&self, name: &str) -> bool {
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

Enable or disable location compression.

```rust
pub fn enable_location_compress(mut self, enable: bool) -> Self {
```

Enable or disable workspace symbol compression.

```rust
pub fn enable_workspace_symbol_compress(mut self, enable: bool) -> Self {
```

Enable or disable workspace diagnostic compression.

```rust
pub fn enable_workspace_diag_compress(mut self, enable: bool) -> Self {
```

Set the output format (json, toon, passthrough).

```rust
pub fn output_format(mut self, fmt: OutputFormat) -> Self {
```

Set the log level.

```rust
pub fn log_level(mut self, level: impl Into<String>) -> Self {
```

Set the response capping limits.

```rust
pub fn capping(mut self, capping: CappingConfig) -> Self {
```

Set the runtime metrics configuration.

```rust
pub fn metrics(mut self, metrics: MetricsConfig) -> Self {
```

Build the [`Config`], validating required fields.

```rust
pub fn build(self) -> Result<Config, LspzError> {
```
