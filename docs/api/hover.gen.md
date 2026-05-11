# API: `crates/lspz-core/src/interceptors/hover`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Compression interceptor for `textDocument/hover`.

Transforms LSP hover responses into a compact format.
On any error, logs a WARN and returns `Err` (fail-open in the chain).

```rust
pub struct HoverCompressor {
```

Whether to compact markdown content (collapse blank lines, shorten fences).

```rust
pub enable_markdown_compact: bool,
```

Compress a hover response value.

```rust
fn compress_hover(params: &Value, markdown_compact: bool) -> Result<Value, LspzError> {
```

Compress the `contents` field of a Hover response.

Handles three formats:
- `MarkupContent`: `{ kind, value }` → `{ k, v }` (with markdown compaction)
- `MarkedString` (string form): `"string"` → `"string"`
- `MarkedString` (object form): `{ language, value }` → `{ l, v }`

```rust
fn compress_contents(contents: &Value, markdown_compact: bool) -> Result<Value, LspzError> {
```

Compress a range value.

```rust
fn compress_range(range: &Value) -> Value {
```

Compress a Position { line, character } → { l, c }.

```rust
fn compress_position(pos: &Value) -> Value {
```

Compress markdown text:
- Collapse 2+ consecutive blank lines → single blank line
- Shorten code fence markers: ` ```language ` → `` `lc `` (first 2 chars), ` ``` ` → `` ` ``
- Trim trailing whitespace on each line

```rust
fn compact_markdown(text: &str) -> String {
```

Map MarkupKind to single char.

```rust
fn encode_markup_kind(kind: &str) -> &str {
```
