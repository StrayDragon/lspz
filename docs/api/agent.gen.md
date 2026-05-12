# API: `src/agent_sdk/agent`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

High-level handle to an LSP server session.

```rust
pub struct AgentHandle {
```

Create a new [`AgentBuilder`].

```rust
pub fn builder() -> AgentBuilder {
```

Open a file in the LSP session (send `didOpen` notification).

```rust
async fn open_file(&mut self, uri: &str) -> Result<String, anyhow::Error> {
```

Run params through the interceptor chain if compression is enabled.

```rust
async fn process_through_chain(
```

Get diagnostics for a file.

```rust
pub async fn get_diagnostics(&mut self, uri: &str) -> Result<String, anyhow::Error> {
```

Get completions at a specific cursor position.

```rust
pub async fn get_completions(
```

Get document symbols.

```rust
pub async fn get_symbols(&mut self, uri: &str) -> Result<String, anyhow::Error> {
```

Get hover information at a specific cursor position.

```rust
pub async fn get_hover(
```

Get references at a specific cursor position.

```rust
pub async fn get_references(
```

Get the definition location of a symbol.

```rust
pub async fn get_definition(
```

Get the implementation locations of a symbol.

```rust
pub async fn get_implementation(
```

Get the type definition location of a symbol.

```rust
pub async fn get_type_definition(
```

Query workspace symbols matching a search term.

```rust
pub async fn get_workspace_symbols(&mut self, query: &str) -> Result<String, anyhow::Error> {
```

Get workspace diagnostics for a file.

```rust
pub async fn get_workspace_diagnostics(&mut self, uri: &str) -> Result<String, anyhow::Error> {
```

Expand compressed diagnostics back to standard LSP format.

```rust
pub fn inflate(compressed_json: &str) -> Result<String, anyhow::Error> {
```

Compress standard diagnostics into compact format.

```rust
pub fn compress(raw_json: &str) -> Result<String, anyhow::Error> {
```

Shut down the LSP server session.

```rust
pub async fn shutdown(mut self) -> Result<(), anyhow::Error> {
```

Builder for [`AgentHandle`].

```rust
#[derive(Default)]
```

Set the backend LSP server command.

```rust
pub fn backend(mut self, cmd: impl Into<String>) -> Self {
```

Set the language identifier.

```rust
pub fn language(mut self, lang: impl Into<String>) -> Self {
```

Enable compression (default: disabled).

```rust
pub fn enable_compression(mut self, enabled: bool) -> Self {
```

Start the LSP server and return an [`AgentHandle`].

```rust
pub async fn start(self) -> Result<AgentHandle, anyhow::Error> {
```

Build an interceptor chain with all compressors enabled.

```rust
fn build_interceptor_chain() -> InterceptorChain {
```
