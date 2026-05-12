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

## v0.4.0 (2026-05-10)

Production hardening & completion compression.

- `CompletionCompressor`: CompletionItemKind 枚举缩减 (1-25 → 单 char), 字段裁剪, doc 去重
- E2E 测试框架 (LspTestHarness) with gopls/rust-analyzer/basedpyright/typescript-language-server
- 4 组压测 fixture (diagnostics, completions, hover, symbols)

## v0.5.0 (2026-05-11)

Hover compression & DocumentSymbol compression.

- `HoverCompressor`: Markdown 空白行折叠, code fence 缩短, MarkupKind 缩减
- `DocumentSymbolCompressor`: SymbolKind 1-26 单字符编码, 递归 children 压缩, 双格式支持 (DocumentSymbol 分层 + SymbolInformation 扁平)

## v0.6.0 (2026-05-11)

Documentation cleanup & verification tooling.

- README/ROADMAP 重写
- `just compress-demo` 一键验证 (展示全部压缩器 token 节省)
- Documentation alignment across all specs/guides

## v0.7.0 (2026-05-11)

TOON output format & benchmark/report system.

- `codec/toon.rs`: TOON (Token-Oriented Object Notation) 输出格式 — 自解释行协议 + 表格
- `--output toon|json|passthrough` CLI flag
- TOON 输出 for: diagnostics, completions, hover, symbols
- Benchmark report system: Criterion 吞吐量 + 压缩比报告 (bytes + tokens)
- `docs/reports/latest.md` 自动生成

## v0.8.0 (2026-05-12)

Response capping & proxy response interception.

- `CappingInterceptor`: 截断 + 压缩正交叠加
- `--max-diags` / `--max-completions` / `--max-symbols` CLI flags
- `CappingConfig` struct + builder + env-var 支持
- Proxy 新增 `pending_requests` 跟踪机制, 支持 response 消息拦截
- CompletionCompressor / HoverCompressor / DocumentSymbolCompressor 现可在 proxy 模式下工作

## v0.9.0 (2026-05-12)

Location compression, TCP/WebSocket transport, runtime metrics, config hot-reload.

- `LocationCompressor`: references/definition/implementation/typeDefinition (URI 去重 + delta range)
- `WorkspaceSymbolCompressor`: workspace/symbol (SymbolKind 复用 + URI 去重)
- `WorkspaceDiagnosticCompressor`: workspace/diagnostic (unchanged 跳过 + 复用 DiagnosticsCompressor)
- TCP transport (`TcpTransport`) — default feature
- WebSocket transport (`WsTransport`) — `transport-websocket` feature
- Runtime metrics: `MetredInterceptor` wrapper + `MetricsSnapshot` (压缩率, 延迟, 计数)
- Config hot-reload: `ConfigWatcher` using `notify` crate + `Arc<RwLock<Config>>`
- Agent SDK 统一到 InterceptorChain (所有 7 个压缩器)
- 176+ tests passing
