# API: `src/transport/mock`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

A mock transport for testing LspSession and AgentHandle.

Responses are enqueued with [`push_response`](MockTransport::push_response)
and returned in FIFO order by [`receive`](Transport::receive).
Messages sent via [`send`](Transport::send) are captured for later inspection.

# Example

```ignore
let mock = MockTransport::new();
mock.push_message(&LspMessage::Notification {
method: "textDocument/publishDiagnostics".into(),
params: serde_json::json!({"diagnostics": []}),
});
```

```rust
pub struct MockTransport {
```

Create a new empty mock transport.

```rust
pub fn new() -> Self {
```

Enqueue a raw framed message to be returned by the next `receive()`.

```rust
pub fn push_response(&self, frame: Vec<u8>) {
```

Enqueue an `LspMessage` to be returned by the next `receive()`.

Convenience wrapper that serializes the message to bytes first.

```rust
pub fn push_message(&self, msg: &LspMessage) -> Result<(), LspzError> {
```

Return all bytes sent via `send()` since creation.

```rust
pub fn sent_messages(&self) -> Vec<Vec<u8>> {
```

Clear all queued responses (resets the response queue).

```rust
pub fn clear_responses(&self) {
```
