# API: `src/codec/json_rpc`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Maximum body size: 16 MB.

```rust
const MAX_BODY_SIZE: usize = 16 * 1024 * 1024;
```

A parsed JSON-RPC 2.0 message.

```rust
#[derive(Debug, Clone, PartialEq)]
```

A request with method + params + id.

```rust
Request {
```

A response to a previous request.

```rust
Response {
```

A notification (method + params, no id).

```rust
Notification { method: String, params: Value },
```

JSON-RPC 2.0 error object.

```rust
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
```

Serialize this message into a Content-Length framed byte buffer.

```rust
pub fn to_bytes(&self) -> Result<Vec<u8>, LspzError> {
```

Convert to a [`serde_json::Value`] (JSON-RPC 2.0 request/response/notification).

```rust
pub fn to_json_value(&self) -> Result<Value, LspzError> {
```

Parse a JSON-RPC 2.0 message from a [`serde_json::Value`].

```rust
pub fn parse_message(val: &Value) -> Result<LspMessage, LspzError> {
```

Parse the `id` field from a JSON object.

```rust
fn parse_id(obj: &serde_json::Map<String, Value>) -> Result<i64, LspzError> {
```

Wrap a JSON value in a `Content-Length: N\r\n\r\n` frame.

```rust
pub fn serialize_frame(body: &Value) -> Result<Vec<u8>, LspzError> {
```

A partially parsed frame: header + body bytes.

```rust
#[derive(Debug)]
```

Parse the next complete frame from a buffer.

Returns the `Frame` and the number of bytes consumed.
Returns `None` if more data is needed.

```rust
pub fn parse_frame(buf: &[u8]) -> Result<Option<(Frame, usize)>, LspzError> {
```

Parse the Content-Length value from the header string.

```rust
fn parse_content_length(header: &str) -> Result<u64, LspzError> {
```

A streaming frame parser that handles partial reads and concatenation.

```rust
pub struct FrameReader {
```

Create a new [`FrameReader`] with an empty internal buffer.

```rust
pub fn new() -> Self {
```

Feed new data into the parser and try to extract a complete frame.

Returns the parsed JSON [`Value`] if a complete frame was found.
The remaining unconsumed data stays in the buffer for the next call.

```rust
pub fn feed(&mut self, data: &[u8]) -> Result<Option<Value>, LspzError> {
```

Parse a [`LspMessage`] from a raw Content-Length framed byte slice.

```rust
pub fn from_frame_bytes(bytes: &[u8]) -> Result<LspMessage, LspzError> {
```

Serialize and return the JSON body bytes (without Content-Length header).

```rust
pub fn to_json_bytes(&self) -> Result<Vec<u8>, LspzError> {
```

Helper: build a Content-Length frame from a JSON body string.

```rust
fn frame(body: &str) -> Vec<u8> {
```

Helper: request body

```rust
fn req_body(method: &str, id: i64) -> String {
```

Helper: notification body

```rust
fn not_body(method: &str) -> String {
```
