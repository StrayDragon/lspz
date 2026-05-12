# API: `crates/lspz-core/src/codec/toon`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Convert compact severity to a full, self-explanatory word.

```rust
fn severity_to_str(s: CompactSeverity) -> &'static str {
```

Format a single `[line, char, line, char]` range as `L:C-L:C`.

```rust
fn format_single_range(r: &[i64; 4]) -> String {
```

Format delta-encoded ranges as space-separated absolute positions.

```rust
fn format_ranges(ranges: &[[i64; 4]]) -> String {
```

Escape a string value for CSV output in TOON tabular format.

Replaces newlines with `\n` to keep rows single-line.
Wraps in double quotes if the value contains comma or double-quote.

```rust
fn escape_csv(s: &str) -> String {
```

Convert [`CompactDiagnostics`] to TOON tabular format.

```toon
uri: file:///src/main.rs
diagnostics[2]{severity,message,code,range,count}:
warning,unused variable: `x`,unused_variables,10:5-10:15,2
error,cannot find value `y`,E0425,20:0-20:10,1
```

```rust
pub fn diagnostics_to_toon(diags: &CompactDiagnostics) -> String {
```

Convert compact completion response to TOON tabular format.

Input is the compact JSON `Value` produced by `CompletionCompressor`.
```toon
completions[2]{label,kind,detail,documentation,deprecated}:
push,function,fn push(&mut self, value: T),Adds element to back,false
pop,function,fn pop(&mut self) -> Option<T>,Removes last element,false
```

```rust
pub fn completions_to_toon(value: &Value) -> Result<String, LspzError> {
```

Map compact completion kind char back to full string.

```rust
fn completion_kind_from_char(k: &str) -> &'static str {
```

Resolve documentation from inline doc or doc_pool reference.

```rust
fn resolve_doc(item: &Value, pool: &[String]) -> String {
```

Convert compact hover response to TOON object format.

Input is the compact JSON `Value` produced by `HoverCompressor`.
```toon
kind: markdown
value: Some description\nwith escaped\nnewlines
```

```rust
pub fn hover_to_toon(value: &Value) -> Result<String, LspzError> {
```

Map compact markup kind char back to full string.

```rust
fn hover_kind_to_str(k: &str) -> &'static str {
```

Format a compact hover range as `L:C-L:C`.

```rust
fn format_hover_range(range: &Value) -> String {
```

Convert compact document symbol response to TOON tabular format.

Input is the compact JSON `Value` produced by `DocumentSymbolCompressor`.

For hierarchical `DocumentSymbol`, children are flattened with
parent-prefixed names. For flat `SymbolInformation`, the `container` field
is included.

```toon
symbols[3]{name,kind,range,detail,container}:
MyStruct,struct,0:0-10:1,A test struct,
MyStruct.field1,field,1:4-1:11,,
main,function,12:0-20:1,,
```

```rust
pub fn symbols_to_toon(value: &Value) -> Result<String, LspzError> {
```

A flattened symbol entry for tabular output.

```rust
struct FlatSymbol {
```

Recursively flatten hierarchical DocumentSymbol items.

Children get parent-prefixed names: `"parent.child"`.

```rust
fn flatten_symbols(items: &[Value], prefix: &str) -> Vec<FlatSymbol> {
```

Map compact symbol kind char back to full string.

```rust
fn symbol_kind_from_char(k: &str) -> &'static str {
```

Format a compact symbol range `{s:{l,c}, e:{l,c}}` to `L:C-L:C`.

```rust
fn format_symbol_range(range: &Value) -> String {
```
