# API: `src/transport/websocket`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Transport over a WebSocket connection.

Connects to a remote LSP server that exposes a WebSocket endpoint.
Supports both `ws://` and `wss://` URLs.

```rust
pub struct WsTransport {
```

Connect to a WebSocket URL.

`url` should be a `ws://` or `wss://` URL (e.g. `"ws://localhost:8080/lsp"`).

```rust
pub async fn connect(url: &str) -> Result<Self, LspzError> {
```
