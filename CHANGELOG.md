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
