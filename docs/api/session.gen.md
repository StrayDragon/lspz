# API: `src/mcp/session`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

A connected LSP server session.

```rust
pub struct LspSession {
```

Spawn an LSP server and return an uninitialized session.

```rust
pub fn spawn(cmd: &str) -> Result<Self, anyhow::Error> {
```

Create a session with a pre-constructed transport.

```rust
pub fn with_transport(transport: Box<dyn Transport>) -> Self {
```

Perform the LSP initialize/initialized handshake.

```rust
pub async fn initialize(&mut self) -> Result<Value, anyhow::Error> {
```

Send a request and wait for the matching response.

```rust
pub async fn send_request(
```

Send a notification (fire-and-forget).

```rust
pub async fn send_notification(
```

Read frames until a notification with the given method arrives.

```rust
pub async fn wait_for_notification(&mut self, method: &str) -> Result<Value, anyhow::Error> {
```

Check if the child process has exited.

```rust
pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, anyhow::Error> {
```
