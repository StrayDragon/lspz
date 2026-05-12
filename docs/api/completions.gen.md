# API: `src/interceptors/completions`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Compression interceptor for `textDocument/completion`.

Transforms LSP completion responses into a compact format.
On any error, logs a WARN and returns `Err` (fail-open in the chain).

```rust
pub struct CompletionCompressor {
```

Maximum number of completion items to keep (default: 50).

```rust
pub max_items: usize,
```

Whether to deduplicate identical documentation strings (default: true).

```rust
pub enable_doc_dedup: bool,
```

Compress a completion response value.

```rust
fn compress_completions(
```

Map LSP CompletionItemKind numeric value to a single character.

See https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#completionItemKind

```rust
fn encode_completion_kind(kind: u64) -> Option<char> {
```
