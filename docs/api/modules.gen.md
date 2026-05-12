# 模块索引

> 自动从 `//!` 模块级注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

- **`examples/bench-report`**: lspz Compression Benchmark Report
- **`examples/compress-demo`**: lspz Compression Demo
- **`src/agent_sdk/agent`**: AgentHandle — high-level LSP integration for AI coding agents.
- **`src/agent_sdk/mod`**: # lspz Agent SDK
- **`src/agent_sdk/pool`**: AgentPool — multi-language LSP session management for AI agents.
- **`src/codec/compact`**: Compact format for LSP diagnostics.
- **`src/codec/json_rpc`**: JSON-RPC 2.0 Content-Length frame parser.
- **`src/codec/mod`**: Message codec layer.
- **`src/codec/toon`**: TOON (Token-Oriented Object Notation) output format.
- **`src/config`**: Runtime configuration.
- **`src/config_watcher`**: File-system watcher for hot-reloading [`Config`](crate::config::Config).
- **`src/error`**: Unified error type for lspz.
- **`src/interceptors/capping`**: Response capping interceptor.
- **`src/interceptors/completions`**: Completion compression interceptor.
- **`src/interceptors/diagnostics`**: Diagnostic compression interceptor.
- **`src/interceptors/hover`**: Hover compression interceptor.
- **`src/interceptors/locations`**: Location / LocationLink compression interceptor.
- **`src/interceptors/mod`**: Interceptor trait and chain.
- **`src/interceptors/symbols`**: DocumentSymbol / SymbolInformation compression interceptor.
- **`src/interceptors/workspace_diagnostics`**: Workspace Diagnostic compression interceptor.
- **`src/interceptors/workspace_symbols`**: Workspace Symbol compression interceptor.
- **`src/lib`**: # lspz
- **`src/main`**: CLI entry point for lspz — LSP compression proxy and MCP server.
- **`src/mcp/mod`**: # lspz MCP server
- **`src/mcp/session`**: LSP session — manages a single LSP server connection.
- **`src/metrics`**: Runtime metrics for the interceptor chain.
- **`src/proxy`**: LSP proxy state machine and message loop.
- **`src/transport/framing`**: Shared Content-Length framing for LSP transports.
- **`src/transport/mock`**: Mock transport for testing LspSession without real LSP servers.
- **`src/transport/mod`**: Transport abstraction.
- **`src/transport/stdio`**: Stdio transport for LSP communication.
- **`src/transport/tcp`**: TCP transport for LSP communication.
- **`src/transport/websocket`**: WebSocket transport for LSP communication.
