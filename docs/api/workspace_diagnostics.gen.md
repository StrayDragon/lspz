# API: `src/interceptors/workspace_diagnostics`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Compression interceptor for `workspace/diagnostic`.

For 'full' entries, reuses the same 5-step pipeline as [`DiagnosticsCompressor`]:
field pruning, message normalization, severity reduction, dedup, range encoding.
For 'unchanged' entries, passes through transparently.

```rust
pub struct WorkspaceDiagnosticCompressor;
```

Top-level compression entry point.

Processes `{ items: [...] }`, compressing each 'full' entry individually.

```rust
fn compress_workspace_diagnostics(params: &Value) -> Result<Value, LspzError> {
```

Compress a single workspace diagnostic document entry.

- `kind: 'unchanged'` → pass through with `_c: "u"` marker
- `kind: 'full'` → compress diagnostics via compact::compress

```rust
fn compress_doc_item(item: &Value) -> Value {
```

Compress a 'full' entry by delegating to the diagnostics compressor.

Constructs a `publishDiagnostics`-shaped `{ uri, diagnostics }` value,
passes it through `compact::compress`, then stitches the result back.

```rust
fn compress_full(item: &Value) -> Value {
```

Mark an unchanged entry with a minimal marker.

```rust
fn compress_unchanged(item: &Value) -> Value {
```

Convert compact workspace diagnostic response to TOON format.

Outputs each file's diagnostics in sequence, with unchanged files noted.

```rust
pub fn workspace_diagnostics_to_toon(value: &Value) -> Result<String, LspzError> {
```

Map compact severity to full string for TOON output.

```rust
fn severity_to_str(d: &Value) -> &'static str {
```

Format a compact diagnostic range to `L:C-L:C`.

```rust
fn format_ws_diag_range(d: &Value) -> String {
```
