# API: `src/interceptors/locations`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Compression interceptor for Location / LocationLink responses.

Covers `textDocument/references`, `textDocument/definition`,
`textDocument/implementation`, and `textDocument/typeDefinition`.
On any error, logs a WARN and returns `Err` (fail-open in the chain).

```rust
pub struct LocationCompressor;
```

Top-level compression entry point.

Normalizes the input to an array, builds a URI pool, then compresses each item.

```rust
fn compress_locations(params: &Value) -> Result<Value, LspzError> {
```

Normalize the params value to a Vec of references.

- `Value::Null` → empty vec
- single object with `uri` or `targetUri` → vec of one element
- `Value::Array` → cloned elements

```rust
fn normalize_to_array(params: &Value) -> Vec<Value> {
```

Extract unique URI strings from an iterator of values.

Each value is expected to be an object with either `uri` (Location) or
`targetUri` (LocationLink). URIs are collected in insertion order; duplicates
are skipped. If an item has neither field, it contributes no URI.

```rust
fn build_uri_pool(items: &[Value]) -> Vec<String> {
```

Compress a single Location or LocationLink value.

Detects LocationLink by checking for `targetUri` field.

```rust
fn compress_item(value: &Value, uri_pool: &[String]) -> Value {
```

Compress a Location: `{ uri, range }` → `{ u: idx, r: { s: { l, c }, e: { l, c } } }`.

```rust
fn compress_location(value: &Value, uri_pool: &[String]) -> Value {
```

Compress a LocationLink: `{ targetUri, targetRange, targetSelectionRange, originSelectionRange? }`
→ `{ u: idx, r, s, o? }`.

```rust
fn compress_location_link(value: &Value, uri_pool: &[String]) -> Value {
```

Compress a Range: `{ start, end }` → `{ s, e }`, each position: `{ line, character }` → `{ l, c }`.

```rust
fn compress_range(range: &Value) -> Value {
```

Compress a Position: `{ line, character }` → `{ l, c }`.

```rust
fn compress_position(pos: &Value) -> Value {
```
