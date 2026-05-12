# API: `src/mcp/server`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

MCP server that exposes LSP functionality as tools.

```rust
#[derive(Clone)]
```

Create a new MCP server with an empty connection pool.

```rust
pub fn new() -> Self {
```
