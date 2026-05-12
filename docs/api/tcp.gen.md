# API: `src/transport/tcp`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Transport over a raw TCP socket.

Connects to a remote LSP server (e.g. `rust-analyzer --port 2087`)
and wraps the socket in Content-Length framed I/O.

```rust
pub struct TcpTransport {
```

Connect to a TCP address.

`addr` should be in the form `host:port` (e.g. `"localhost:2087"`).

```rust
pub async fn connect(addr: &str) -> Result<Self, LspzError> {
```

Echo server that responds with Content-Length framed messages.

```rust
async fn echo_server(addr: &str) -> Result<(), std::io::Error> {
```
