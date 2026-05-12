# API: `crates/lspz-core/src/interceptors/symbols`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

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
pub(crate) fn compress_location(location: &Value) -> Value {
```

Compress a Range: `{ start, end }` → `{ s, e }`, each Position compacted to `{ l, c }`.

```rust
pub(crate) fn compress_range(range: &Value) -> Value {
```

Compress a Position: `{ line, character }` → `{ l, c }`.

```rust
pub(crate) fn compress_position(pos: &Value) -> Value {
```

Map LSP SymbolKind numeric value to a single character.

See <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#symbolKind>

```rust
pub(crate) fn encode_symbol_kind(kind: u64) -> Option<char> {
```
