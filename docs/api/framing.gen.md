# API: `src/transport/framing`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Read one complete Content-Length framed LSP message.

Reads headers byte-by-byte until `\r\n\r\n`, parses `Content-Length`,
then reads exactly that many body bytes. Returns the full frame (header + body).

```rust
pub(crate) async fn read_frame<R: AsyncRead + Unpin>(
```

Parse `Content-Length` from the header string.

```rust
pub(crate) fn parse_content_length(header: &str) -> Result<u64, LspzError> {
```

Read a frame from an in-memory buffer.

```rust
#[tokio::test]
```
