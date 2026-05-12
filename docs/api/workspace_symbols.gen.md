# API: `crates/lspz-core/src/interceptors/workspace_symbols`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Compression interceptor for `workspace/symbol` responses.

Reuses SymbolKind encoding from [`DocumentSymbolCompressor`] and URI pooling
from [`LocationCompressor`] for maximum token savings.

```rust
pub struct WorkspaceSymbolCompressor;
```

Top-level compression entry point.

```rust
fn compress_workspace_symbols(params: &Value) -> Result<Value, LspzError> {
```

Build URI pool from SymbolInformation items (extract `location.uri`).

```rust
fn build_uri_pool(items: &[Value]) -> Vec<String> {
```

Compress a single SymbolInformation entry.

Field mapping:
- `name` → `n`
- `kind` → `k` (single char via encode_symbol_kind)
- `containerName` → `c`
- `location` → `l`: `{ uri, range }` → `{ u: pool_idx, r: { s, e } }`
- Dropped: `deprecated`, `tags`, `data`

```rust
fn compress_symbol_item(value: &Value, uri_pool: &[String]) -> Value {
```

Convert compact workspace symbol response to TOON tabular format.

```rust
pub fn workspace_symbols_to_toon(value: &Value) -> Result<String, LspzError> {
```

Map compact symbol kind char back to full string.

```rust
fn symbol_kind_from_char(k: &str) -> &'static str {
```

Format a compact range `{ s: { l, c }, e: { l, c } }` to `L:C-L:C`.

```rust
fn format_ws_range(range: &Value) -> String {
```

Escape a string for CSV output.

```rust
fn escape_csv(s: &str) -> String {
```
