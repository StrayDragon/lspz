# API: `lspz-core/src/codec/compact`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Severity encoded as a single character.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
```

Convert from LSP severity number (1–4).

```rust
pub fn from_lsp(severity: u64) -> Option<Self> {
```

Convert back to LSP severity number (1–4).

```rust
pub fn to_lsp(self) -> u64 {
```

A single compact diagnostic entry.

Multiple original diagnostics with identical (message, severity, code)
are merged into one entry with multiple ranges.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
```

Normalized message text.

```rust
pub m: String,
```

Single-char severity.

```rust
pub s: CompactSeverity,
```

Ranges: first element is absolute, subsequent ones are delta-encoded.

```rust
pub r: Vec<[i64; 4]>,
```

Optional diagnostic code (helps AI categorise errors).

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
```

Optional tags (comma-separated: "U" / "D" / "U,D").

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
```

Number of merged diagnostics (default 1).

```rust
#[serde(default, skip_serializing_if = "is_one")]
```

Top-level compact diagnostics message.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
```

Format version (currently 1).

```rust
pub version: u32,
```

Document URI.

```rust
pub uri: String,
```

Compacted diagnostic entries.

```rust
pub diagnostics: Vec<CompactDiagnostic>,
```

Encode a single LSP range into `[line_start, char_start, line_end, char_end]`.

Input is the LSP `range` value: `{"start": {"line": L, "character": C}, "end": {"line": L, "character": C}}`.

```rust
pub fn encode_range(range: &Value) -> Option<[i64; 4]> {
```

Encode multiple ranges with delta compression.

First range is absolute, subsequent ranges are deltas from the previous one.

```rust
pub fn encode_ranges_with_delta(ranges: Vec<Value>) -> Vec<[i64; 4]> {
```

Decode a delta-encoded range back to absolute values.

`prev` is the previous absolute range (used for delta accumulation).

```rust
pub fn decode_delta_range(delta: &[i64; 4], prev: &[i64; 4]) -> [i64; 4] {
```

Reconstruct all absolute ranges from a delta-encoded array.

```rust
pub fn decode_ranges(encoded: &[[i64; 4]]) -> Vec<[i64; 4]> {
```

Encode LSP diagnostic tags to a compact single-character format.

- `Unnecessary` (1) → `"U"`
- `Deprecated` (2) → `"D"`
- Multiple tags → `"U,D"`

```rust
pub fn encode_tags(tags: &[Value]) -> Option<String> {
```

Result of a single compression step.

```rust
#[derive(Debug)]
```

Compress LSP diagnostic params into the compact format.

`params` is the value of a `textDocument/publishDiagnostics` notification:
`{"uri": "...", "diagnostics": [...]}`.

```rust
pub fn compress(params: &Value) -> Result<Value, LspzError> {
```

Decompress a `CompactDiagnostics` value back to LSP `publishDiagnostics` params.

```rust
pub fn decompress(compact_val: &Value) -> Result<Value, LspzError> {
```

A dedup key used during compression grouping.

```rust
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
```

Group diagnostics by normalised key for dedup.

```rust
pub fn group_by_key(entries: Vec<CompressedEntry>) -> Vec<CompressedEntry> {
```
