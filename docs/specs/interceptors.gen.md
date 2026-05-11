# 拦截器参考

> 自动从 `Interceptor` trait 定义及实现生成。编辑源码后运行 `just gen-api-docs` 刷新。

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

Compression interceptor for `textDocument/publishDiagnostics`.

Transforms LSP diagnostics into the compact format.
On any error, logs a WARN and returns `Err` (fail-open in the chain).

```rust
pub struct DiagnosticsCompressor {
```

Whether to apply message normalisation before dedup.

```rust
pub enable_normalisation: bool,
```

Normalize a diagnostic message by stripping variable-specific content.

This is the critical enabler for dedup: without normalization, messages like
`"declared and not used: foo"` and `"declared and not used: bar"` can't merge.

## Known patterns

| code | Raw message | Normalised |
|------|-------------|------------|
| `unused_var` / `UnusedVar` | `"declared and not used: x"` | `"unused variable"` |
| `unused_import` / `UnusedImport` | `"\"os\" imported and not used"` | `"unused import"` |
| `reportUnusedVariable` | `"Variable \"x\" is not used"` | `"unused variable"` |
| `reportUnusedImport` | `"Import \"os\" is unused"` | `"unused import"` |
| `6133` (TypeScript) | `"'temp' is declared but never read"` | `"unused variable"` |
| _any_ with backtick identifiers | `` "use `foo`" `` | `` "use `<ident>`" `` |

```rust
pub fn normalize_message(message: &str, code: Option<&str>) -> String {
```

Normalize by TypeScript-style numeric diagnostic codes.

```rust
fn normalize_by_numeric_code(code: i64) -> Option<&'static str> {
```

Check if the code represents an "unused variable" diagnostic.

```rust
fn is_unused_var_code(code: &str) -> bool {
```

Check if the code represents an "unused import" diagnostic.

```rust
fn is_unused_import_code(code: &str) -> bool {
```

Replace backtick-wrapped identifiers with a placeholder.

`` "use `foo`" `` → `` "use `<ident>`" ``

```rust
fn replace_backtick_idents(s: &str) -> String {
```

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

Direction of an LSP message.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
```

Client → Server

```rust
ClientToServer,
```

Server → Client

```rust
ServerToClient,
```

A single interceptor in the chain.

[MermaidChart:./docs/mmd/interceptor-chain.mmd]

```rust
#[async_trait::async_trait]
```

Unique name for logging / configuration.

```rust
fn name(&self) -> &str;
```

Whether this interceptor should process the given message.

```rust
fn applies_to(&self, method: &str, direction: Direction) -> bool;
```

Transform the message params.

Returns:
- `Ok(Some(params))` — modified params to use
- `Ok(None)` — drop the message
- `Err(_)` — fail open; caller should forward original

```rust
async fn intercept(
```

A chain of interceptors executed in order.

```rust
pub struct InterceptorChain {
```

Create a new chain with the given interceptors.

```rust
pub fn new(interceptors: Vec<Box<dyn Interceptor>>) -> Self {
```

Process a message through all matching interceptors.

```rust
pub async fn process(
```

An interceptor that always fails.

```rust
struct AlwaysFailInterceptor;
```

Compression interceptor for `textDocument/documentSymbol`.

Transforms LSP document symbol responses into a compact format.
On any error, logs a WARN and returns `Err` (fail-open in the chain).

```rust
pub struct DocumentSymbolCompressor;
```

Compress a `textDocument/documentSymbol` response value.

Handles both `DocumentSymbol[]` (hierarchical, has `range`) and
`SymbolInformation[]` (flat, has `location`) on a per-element basis.

```rust
fn compress_symbols(params: &Value) -> Result<Value, LspzError> {
```

Compress a single symbol, detecting whether it is a DocumentSymbol or SymbolInformation.

```rust
fn compress_symbol(value: &Value) -> Value {
```

Compress a hierarchical DocumentSymbol entry.

Field mapping:
- `name` → `n`, `kind` → `k` (char), `range` → `r`, `detail` → `d`, `children` → `c`
- Dropped: `deprecated`, `tags`, `selectionRange`

```rust
fn compress_document_symbol(value: &Value) -> Value {
```

Compress a flat SymbolInformation entry.

Field mapping:
- `name` → `n`, `kind` → `k` (char), `location` → `l`, `containerName` → `cn`
- Dropped: `deprecated`, `tags`

```rust
fn compress_symbol_information(value: &Value) -> Value {
```

Compress a Location: `{ uri, range }` → `{ u, r }`.

```rust
fn compress_location(location: &Value) -> Value {
```

Compress a Range: `{ start, end }` → `{ s, e }`, each Position compacted to `{ l, c }`.

```rust
fn compress_range(range: &Value) -> Value {
```

Compress a Position: `{ line, character }` → `{ l, c }`.

```rust
fn compress_position(pos: &Value) -> Value {
```

Map LSP SymbolKind numeric value to a single character.

See <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#symbolKind>

```rust
fn encode_symbol_kind(kind: u64) -> Option<char> {
```
