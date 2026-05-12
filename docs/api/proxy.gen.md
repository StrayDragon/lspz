# API: `crates/lspz-core/src/proxy`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Proxy state machine states.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
```

Initial state before [`Proxy::start`] is called.

```rust
Created,
```

Performing LSP initialize/initialized handshake.

```rust
Initializing,
```

Handshake complete, message loop running.

```rust
Ready,
```

Shutdown requested, draining remaining messages.

```rust
ShuttingDown,
```

Fully exited.

```rust
Exited,
```

The lspz proxy.

Combines a client-side I/O (stdin/stdout) with a server-side [`Transport`]
and an [`InterceptorChain`] for Server→Client message transformation.

```rust
pub struct Proxy {
```

Tracks in-flight request IDs to their method for response interception.

```rust
pending_requests: HashMap<u64, String>,
```

Create a new [`Proxy`].

```rust
pub fn new(
```

Returns the current [`State`].

```rust
pub fn state(&self) -> State {
```

Start the proxy: handshake → message loop.

```rust
pub async fn start(&mut self) -> Result<(), LspzError> {
```

LSP initialize/initialized handshake.

```rust
async fn perform_handshake(&mut self) -> Result<(), LspzError> {
```

Main message loop using [`tokio::select!`].

- Client → Server: transparent forward
- Server → Client: interceptor chain → forward

```rust
async fn message_loop(&mut self) -> Result<(), LspzError> {
```

Process a raw server→client message through the interceptor chain.

Supports both notifications (with `method`) and responses (with `id`).
Returns the (possibly transformed) frame bytes, or empty if dropped.
Always succeeds: on error, returns the original raw bytes (fail-open).

```rust
async fn process_server_message(&mut self, raw: &[u8]) -> Vec<u8> {
```

Process a notification or server→client request through the interceptor chain.

```rust
async fn process_notification(
```

Process a server→client response (no method, has id).

Looks up the original request method from `pending_requests`,
passes the response `result` through the interceptor chain,
then reconstructs the response.

```rust
async fn process_response(&mut self, json_val: &serde_json::Value, raw: &[u8]) -> Vec<u8> {
```

Convert transformed params to TOON format and wrap in JSON-RPC.

```rust
fn toon_output(&self, method: &str, params: &serde_json::Value, raw: &[u8]) -> Vec<u8> {
```

Track a client→server request ID → method mapping for response interception.

```rust
fn track_pending_request(raw: &[u8], pending: &mut HashMap<u64, String>) {
```

Read one complete Content-Length framed message from stdin.

```rust
async fn read_stdin_frame(reader: &mut BufReader<tokio::io::Stdin>) -> Result<Vec<u8>, LspzError> {
```

Parse Content-Length from an LSP header.

```rust
fn parse_content_length(header: &str) -> Result<u64, LspzError> {
```

Extract the LSP method name from a raw framed message.

```rust
fn extract_method(raw: &[u8]) -> Result<String, LspzError> {
```

Ensure a raw message has the expected method name.

```rust
fn ensure_method(raw: &[u8], expected: &str) -> Result<(), LspzError> {
```
