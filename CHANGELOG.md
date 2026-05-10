# Changelog

## v0.1.0 (2026-05-10)

LSP diagnostic compression proxy MVP — Library and Proxy modes.

### v0.1.0-alpha.1 — Workspace + JSON-RPC Codec + Transport

- Cargo workspace with `lspz-core` (library) and `lspz` (CLI) crates
- Custom JSON-RPC 2.0 codec with `Content-Length` frame parsing and streaming `FrameReader`
- Transport abstraction (`Transport` trait) with `StdioTransport` implementation for child process LSP servers
- 22 unit tests covering codec frame parsing, streaming, edge cases, and transport header parsing

### v0.1.0-alpha.2 — Proxy State Machine + CLI

- Proxy state machine: `Created → Initializing → Ready ↔ Working → ShuttingDown → Exited`
- Initialization handshake (`initialize`/`initialized`) with transparent forwarding
- `tokio::select!`-based message loop for concurrent client/server I/O
- CLI entry point via clap with `--backend`, `--compress`, `--log-level` flags and env-var overrides
- Unified error type (`LspzError`) with thiserror
- `Config` struct with builder pattern and env-var fallbacks
- 6 unit tests for proxy helpers and config validation

### v0.1.0-rc.1 — Diagnostic Compression

- `Interceptor` trait and `InterceptorChain` for modular Server→Client message transformation
- `DiagnosticsCompressor`: 5-step compression pipeline
  - Field pruning (drops `source`, `data`, `codeDescription`, `relatedInformation`)
  - Message normalization (backtick identifier replacement, code-based pattern recognition)
  - Enum reduction (severity → `E`/`W`/`I`/`H`, compact field names)
  - Dedup via grouping by `(normalized_message, severity, code)`
  - Delta range encoding (first absolute, subsequent as deltas)
- `CompactFormat` codec with `compress`/`decompress`, delta encoding, roundtrip guarantees
- Fail-open: compression errors WARN-log and transparently forward original messages
- SSOT documentation generation via `scripts/gen-docs.py` with drift detection (`--check` mode)
- 10 unit tests for compression pipeline, 9 for compact format codec

### v0.1.0 — Integration Tests + Docs + Release

- Integration tests with realistic gopls (13 diagnostics) and rust-analyzer (3 diagnostics) fixtures
- Dedup verification: 9 unused vars → 1 group, 2 unused imports → 1 group
- Token savings verification: ≥30% basic compression, ≥60% with normalization (gopls), ≥40% (rust-analyzer)
- Normalization effectiveness test (dedup improves with normalization enabled)
- Compression roundtrip tests (compress → decompress preserves all diagnostics)
- Full QA pipeline: `just qa` (fmt + clippy + test + gen-check + prek hooks)
- `just gen-docs` / `just gen-check` for SSOT documentation lifecycle
- `CHANGELOG.md` and project documentation

## v0.2.0 (2026-05-10)

MCP server mode — LSP diagnostics, completions, and symbols as MCP tools.

### Highlights

- New `lspz-mcp` crate (v0.2.0) with `McpServer`, `LspSession`, and `LspPool`
- Three MCP tools via `rmcp` 0.16 SDK:
  - `get_diagnostics` — returns compressed LSP diagnostics for a file
  - `get_completions` — returns completion items at a cursor position
  - `get_symbols` — returns document symbols
- `LspSession` wraps `StdioTransport` with LSP initialize/initialized handshake, `send_request`, `send_notification`, and `wait_for_notification`
- `LspPool` provides lazy multi-language session caching (`HashMap<String, LspSession>`)
- CLI `lspz mcp` subcommand to start the MCP stdio server
- Claude Desktop integration via `claude_desktop_config.json`

## v0.3.0 (2026-05-10)

Agent SDK — declarative LSP integration for AI coding agents.

### Highlights

- New `lspz-agent-sdk` crate (v0.3.0) with `AgentHandle` and `AgentPool`
- `AgentHandle` builder API for single-language LSP sessions:
  - `get_diagnostics` with optional compact compression
  - `get_completions`, `get_symbols`
  - `inflate`/`compress` helpers
  - `shutdown` for clean LSP shutdown handshake
- `AgentPool` for multi-language session management (lazy spawn, per-language dispatch)
- `MockTransport` for testing LSP sessions without real server processes
  - Pre-program responses via `push_message`, inspect sent data via `sent_messages`
  - 5 unit tests for MockTransport, 11 for AgentHandle, 5 for AgentPool
- `LspSession` refactored to `Box<dyn Transport>` for testability
- CI/CD pipeline (GitHub Actions: fmt + clippy + test + gen-check)
- License: MIT
- Synthetic fixture tests for basedpyright and typescript-language-server diagnostic formats
- Claude Desktop integration guide (`docs/guides/claude-desktop-integration.md`)
