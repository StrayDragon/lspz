# API: `lspz-core/src/interceptors/diagnostics`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

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
| _any_ with backtick identifiers | `` "use `foo`" `` | `` "use `<ident>`" `` |

```rust
pub fn normalize_message(message: &str, code: Option<&str>) -> String {
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
