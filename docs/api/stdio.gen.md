# API: `lspz-core/src/transport/stdio`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Transport over a child process's stdio.

Spawns the backend LSP server and wraps its stdin/stdout in an async I/O layer.

```rust
pub struct StdioTransport {
```

Spawn a new process and connect to its stdio.

The `cmd` is split on whitespace into program + arguments.

```rust
pub fn spawn(cmd: &str) -> Result<Self, LspzError> {
```

Check if the child process has exited.

```rust
pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, LspzError> {
```

Parse Content-Length from the accumulated header bytes.

```rust
fn parse_content_length_from_header(header: &str) -> Result<u64, LspzError> {
```
