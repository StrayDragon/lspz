# Agent SDK Integration Guide

The `lspz` crate (feature = "agent-sdk") provides a high-level API for embedding LSP capabilities
into AI coding agents. It manages LSP server process lifecycle, file synchronization,
and provides type-safe query methods.

## Overview

```
┌─────────────────────────────────────────┐
│         Your AI Agent CLI               │
│    cargo add lspz --no-default-features --features agent-sdk  │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│         lspz (agent-sdk)                 │
│  AgentHandle (single language)          │
│  AgentPool   (multi language)           │
│  → get_diagnostics / get_completions    │
│  → get_symbols / get_hover              │
│  → get_references / get_definition      │
│  → get_implementation / get_type_def    │
│  → get_workspace_symbols / diagnostics  │
│  → inflate / compress                   │
└────────────────┬────────────────────────┘
                 │ delegates to
┌────────────────▼────────────────────────┐
│    lspz::mcp :: LspSession              │
│  → spawn → initialize → send_request    │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│    lspz::codec::compact                  │
│  → compress / decompress (token saving) │
└─────────────────────────────────────────┘
```

## Quick Start (Single Language)

Add the dependency:

```toml
[dependencies]
lspz = { version = "0.9", default-features = false, features = ["agent-sdk"] }
tokio = { version = "1.35", features = ["full"] }
anyhow = "1.0"
```

Basic usage:

```rust,no_run
use lspz::agent_sdk::AgentHandle;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Start a rust-analyzer session
    let mut agent = AgentHandle::builder()
        .backend("rust-analyzer")
        .language("rust")
        .start()
        .await?;

    // Get diagnostics for a file
    let diags = agent
        .get_diagnostics("file:///home/user/project/src/main.rs")
        .await?;
    println!("Diagnostics: {diags}");

    // Get completions at a cursor position
    let completions = agent
        .get_completions("file:///home/user/project/src/main.rs", 42, 10)
        .await?;
    println!("Completions: {completions}");

    // Get document symbols
    let symbols = agent
        .get_symbols("file:///home/user/project/src/main.rs")
        .await?;
    println!("Symbols: {symbols}");

    // Get hover information at a position
    let hover = agent
        .get_hover("file:///home/user/project/src/main.rs", 10, 5)
        .await?;
    println!("Hover: {hover}");

    // Go to definition
    let def = agent
        .get_definition("file:///home/user/project/src/main.rs", 10, 5)
        .await?;
    println!("Definition: {def}");

    // Find all references
    let refs = agent
        .get_references("file:///home/user/project/src/main.rs", 10, 5)
        .await?;
    println!("References: {refs}");

    // Clean shutdown
    agent.shutdown().await?;
    Ok(())
}
```

## Compression

Enable compression for token savings (40-95% depending on message type):

```rust,no_run
let mut agent = AgentHandle::builder()
    .backend("rust-analyzer")
    .language("rust")
    .enable_compression(true)  // ← compact format
    .start()
    .await?;

let compressed = agent.get_diagnostics("file:///src/main.rs").await?;

// Decompress back to standard LSP format when needed
let expanded = AgentHandle::inflate(&compressed)?;
```

The compact format uses (for all 7 compressors):
- Range array encoding (`[line, col, line, col]` instead of full objects)
- Severity mapping (`E`/`W`/`I`/`H`), SymbolKind (1-26 → single char)
- URI deduplication (shared pool for location/workspace queries)
- Delta range encoding for multiple diagnostics
- Dedup grouping by normalized message
- Field pruning (drops `source`, `data`, `tags`, `deprecated`, etc.)
- Doc string interning (shared documentation strings)

## Multi-Language Support (AgentPool)

For projects that span multiple languages:

```rust,no_run
use lspz::agent_sdk::AgentPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Register multiple language backends
    let mut pool = AgentPool::builder()
        .register("rust", "rust-analyzer")
        .register("go", "gopls")
        .register("typescript", "typescript-language-server --stdio")
        .enable_compression(true)
        .start_all()
        .await?;

    // Query different languages
    let rust_diags = pool
        .get_diagnostics("file:///src/main.rs", "rust")
        .await?;

    let go_diags = pool
        .get_diagnostics("file:///src/main.go", "go")
        .await?;

    let ts_symbols = pool
        .get_symbols("file:///src/app.ts", "typescript")
        .await?;

    // Shutdown all sessions
    pool.shutdown_all().await?;
    Ok(())
}
```

Sessions are lazily spawned on first use and cached for subsequent queries.

## Supported LSP Operations

### File-scoped queries (require URI)

| Method | LSP Method | Compression |
|--------|-----------|-------------|
| `get_diagnostics(uri)` | `textDocument/didOpen` + wait for `publishDiagnostics` | ✅ DiagnosticsCompressor |
| `get_completions(uri, line, char)` | `textDocument/completion` | ✅ CompletionCompressor |
| `get_symbols(uri)` | `textDocument/documentSymbol` | ✅ DocumentSymbolCompressor |
| `get_hover(uri, line, char)` | `textDocument/hover` | ✅ HoverCompressor |
| `get_references(uri, line, char)` | `textDocument/references` | ✅ LocationCompressor |
| `get_definition(uri, line, char)` | `textDocument/definition` | ✅ LocationCompressor |
| `get_implementation(uri, line, char)` | `textDocument/implementation` | ✅ LocationCompressor |
| `get_type_definition(uri, line, char)` | `textDocument/typeDefinition` | ✅ LocationCompressor |

### Workspace-scoped queries

| Method | LSP Method | Compression |
|--------|-----------|-------------|
| `get_workspace_symbols(query)` | `workspace/symbol` | ✅ WorkspaceSymbolCompressor |
| `get_workspace_diagnostics(uri)` | `workspace/diagnostic` | ✅ WorkspaceDiagnosticCompressor |

### Utility methods

| Method | Description |
|--------|-------------|
| `compress(raw_json)` | Compress standard LSP diagnostics to compact format |
| `inflate(compressed_json)` | Decompress compact format back to standard LSP |
| `shutdown()` | Send `shutdown` + `exit` per LSP spec |

## URI Format

All query methods require `file://` URIs with absolute paths:

```rust
// ✅ Correct
agent.get_diagnostics("file:///home/user/project/src/main.rs").await?;

// ❌ Will fail — missing file:// prefix
agent.get_diagnostics("/home/user/project/src/main.rs").await?;
```

## Error Handling

All methods return `Result<_, anyhow::Error>`. Common error cases:

- **Missing backend/language**: `AgentBuilder::start()` returns an error if `backend` or `language` is not set
- **Invalid URI**: Non-`file://` URIs return an error immediately (before any LSP communication)
- **File not found**: `tokio::fs::read_to_string` errors propagate if the file doesn't exist
- **LSP protocol errors**: `send_request` returns an error if the LSP server responds with an error
- **Server crash**: Transport errors propagate if the LSP server process exits unexpectedly

## Best Practices

1. **Reuse AgentHandle/AgentPool**: Create once and reuse across queries. Avoid spawning a new session per query.
2. **Enable compression for diagnostics**: The compact format saves ≥40% tokens with no semantic loss.
3. **Shutdown cleanly**: Always call `shutdown()` / `shutdown_all()` to send the proper LSP shutdown handshake.
4. **One AgentHandle per language**: For single-language projects, use `AgentHandle`. For multi-language, use `AgentPool`.
5. **File URIs**: Always use absolute paths with `file://` prefix.

## Examples

Complete examples are available in the repository:

- `examples/agent_demo.rs` — Single-language demo with rust-analyzer

## Reference

- [lspz README](../../README.md)
- [lspz API docs](https://docs.rs/lspz)
- [Compact Format Spec](../specs/002-compression-format.md)
- [LSP Compatibility](../specs/003-lsp-compatibility.md)
- [Testing Guide](./testing-guide.md)
