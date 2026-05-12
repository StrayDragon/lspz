# API: `crates/lspz-core/src/interceptors/capping`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Response capping interceptor.

Placed at the front of the interceptor chain (before compressors).
A limit of 0 means no cap for that type.

```rust
pub struct CappingInterceptor {
```

Maximum diagnostics to keep (0 = unlimited).

```rust
pub max_diags: usize,
```

Maximum completion items to keep (0 = unlimited).

```rust
pub max_completions: usize,
```

Maximum document symbols to keep (0 = unlimited).

```rust
pub max_symbols: usize,
```

Create a new `CappingInterceptor` with the given limits.

```rust
pub fn new(max_diags: usize, max_completions: usize, max_symbols: usize) -> Self {
```

Truncate an array field in a JSON object to at most `limit` items.

If `limit` is 0, no truncation is performed.

```rust
fn cap_array_field(params: &mut Value, field: &str, limit: usize) -> Result<(), LspzError> {
```

Truncate a bare array or `CompletionList` items to at most `limit` items.

```rust
fn cap_completions(params: &mut Value, limit: usize) -> Result<(), LspzError> {
```

Truncate an optional array reference to at most `limit` items.

```rust
fn cap_array(arr: Option<&mut Vec<Value>>, limit: usize) -> Result<(), LspzError> {
```
