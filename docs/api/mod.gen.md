# API: `crates/lspz-core/src/transport/mod`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Abstract I/O channel for LSP communication.

```rust
#[async_trait::async_trait]
```

Receive one raw LSP message (Content-Length framed).

```rust
async fn receive(&mut self) -> Result<Vec<u8>, LspzError>;
```

Send raw bytes to the LSP server/client.

```rust
async fn send(&mut self, data: &[u8]) -> Result<(), LspzError>;
```
