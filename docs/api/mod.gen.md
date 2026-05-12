# API: `src/transport/mod`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

This module is only available with the `transport-websocket` feature.

```rust
#[cfg(feature = "transport-websocket")]
```

Abstract I/O channel for LSP communication.

All LSP message I/O (regardless of transport protocol) is defined by this trait.
Implementations handle Content-Length framing internally and expose raw framed bytes.

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

Check whether the underlying process has exited.

Returns `Ok(None)` by default for non-process transports (mock, TCP, WebSocket).

```rust
fn try_wait(&mut self) -> Result<Option<ExitStatus>, LspzError> {
```
