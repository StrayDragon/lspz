# API: `src/mcp/pool`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

A pool of LSP sessions, keyed by a user-defined name (typically a language
identifier like `"go"` or `"rust"`).

Sessions are created lazily on first access.

```rust
pub struct LspPool {
```

Create an empty pool.

```rust
pub fn new() -> Self {
```

Get or create a session for the given key.

```rust
pub async fn get_or_spawn(
```
