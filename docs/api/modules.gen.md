# 模块索引

> 自动从 `//!` 模块级注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

- **`lspz/src/main`**: CLI entry point for lspz — LSP compression proxy.
- **`lspz-core/src/codec/compact`**: Compact format for LSP diagnostics.
- **`lspz-core/src/codec/json_rpc`**: JSON-RPC 2.0 Content-Length frame parser.
- **`lspz-core/src/codec/mod`**: Message codec layer.
- **`lspz-core/src/config`**: Runtime configuration.
- **`lspz-core/src/error`**: Unified error type for lspz.
- **`lspz-core/src/interceptors/diagnostics`**: Diagnostic compression interceptor.
- **`lspz-core/src/interceptors/mod`**: Interceptor trait and chain.
- **`lspz-core/src/lib`**: # lspz-core
- **`lspz-core/src/proxy`**: LSP proxy state machine and message loop.
- **`lspz-core/src/transport/mod`**: Transport abstraction.
- **`lspz-core/src/transport/stdio`**: Stdio transport for LSP communication.
