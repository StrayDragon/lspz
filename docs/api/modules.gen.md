# 模块索引

> 自动从 `//!` 模块级注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

- **`crates/lspz/src/main`**: CLI entry point for lspz — LSP compression proxy and MCP server.
- **`crates/lspz-core/src/codec/compact`**: Compact format for LSP diagnostics.
- **`crates/lspz-core/src/codec/json_rpc`**: JSON-RPC 2.0 Content-Length frame parser.
- **`crates/lspz-core/src/codec/mod`**: Message codec layer.
- **`crates/lspz-core/src/codec/toon`**: TOON (Token-Oriented Object Notation) output format.
- **`crates/lspz-core/src/config`**: Runtime configuration.
- **`crates/lspz-core/src/error`**: Unified error type for lspz.
- **`crates/lspz-core/src/interceptors/capping`**: Response capping interceptor.
- **`crates/lspz-core/src/interceptors/completions`**: Completion compression interceptor.
- **`crates/lspz-core/src/interceptors/diagnostics`**: Diagnostic compression interceptor.
- **`crates/lspz-core/src/interceptors/hover`**: Hover compression interceptor.
- **`crates/lspz-core/src/interceptors/mod`**: Interceptor trait and chain.
- **`crates/lspz-core/src/interceptors/symbols`**: DocumentSymbol / SymbolInformation compression interceptor.
- **`crates/lspz-core/src/lib`**: # lspz-core
- **`crates/lspz-core/src/proxy`**: LSP proxy state machine and message loop.
- **`crates/lspz-core/src/transport/mock`**: Mock transport for testing LspSession without real LSP servers.
- **`crates/lspz-core/src/transport/mod`**: Transport abstraction.
- **`crates/lspz-core/src/transport/stdio`**: Stdio transport for LSP communication.
