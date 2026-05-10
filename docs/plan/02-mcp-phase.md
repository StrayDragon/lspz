# lspz MCP 集成阶段计划 (v0.2)

**阶段**: Phase 2 - MCP 集成
**版本**: v0.2.0
**状态**: ✅ 已完成
**实际周期**: 单次会话完成

## 目标

实现 lspz 的 **MCP Server Mode**，通过 Model Context Protocol 将 LSP 功能暴露为 MCP tools，
支持 AI 助手（Claude Code 等）直接调用 LSP 诊断、补全和符号查询。

### 核心交付物

1. **lspz-mcp** (v0.2.0) - MCP server crate
2. **lspz mcp** CLI 子命令
3. 三个 MCP tools: get_diagnostics, get_completions, get_symbols
4. LSP 连接池支持多语言场景

---

## 架构

```
┌─────────────────────────────────────────────────┐
│                  AI Client                       │
│          (Claude Code / MCP host)                │
└──────────────────────┬──────────────────────────┘
                       │ MCP (stdio)
┌──────────────────────▼──────────────────────────┐
│              lspz mcp (CLI)                      │
│                                                   │
│  ┌─────────────────────────────────────────────┐  │
│  │         McpServer (ServerHandler)            │  │
│  │  - list_tools → 3 tool definitions           │  │
│  │  - call_tool → route to handlers             │  │
│  └────────────────────┬────────────────────────┘  │
│                       │                            │
│  ┌────────────────────▼────────────────────────┐  │
│  │              LspPool                         │  │
│  │  HashMap<language, LspSession>               │  │
│  │  get_or_spawn → lazy init per language       │  │
│  └────────────────────┬────────────────────────┘  │
│                       │                            │
│  ┌────────────────────▼────────────────────────┐  │
│  │              LspSession                      │  │
│  │  send_request / send_notification            │  │
│  │  wait_for_notification                       │  │
│  └────────────────────┬────────────────────────┘  │
└───────────────────────┬──────────────────────────┘
                        │ LSP (stdio)
┌───────────────────────▼──────────────────────────┐
│         Backend LSP Server                        │
│    (rust-analyzer / gopls / ...)                  │
└──────────────────────────────────────────────────┘
```

## 技术决策

### 核心依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| rmcp | 0.16 | 官方 Rust MCP SDK (modelcontextprotocol/rust-sdk) |
| tokio | workspace | 异步运行时 |
| lspz-core | path | 共享 codec、transport |

### 关键设计

1. **手动 ServerHandler 实现**：rmcp v0.16 的 RPITIT trait 不支持 `#[async_trait]`，选择直接实现 `impl Future` 签名
2. **手动 JSON Schema**：避免引入 schemars 依赖，使用 `rmcp::model::object()` 将 `serde_json::json!()` 转换为 `JsonObject`
3. **LSP 连接池**：`LspPool` 按 `language` key 缓存 `LspSession`，首次请求时惰性 spawn+initialize

## MCP Tools

### get_diagnostics

- **输入**: `{ uri, backend, language }`
- **流程**: didOpen → wait_for_notification(publishDiagnostics) → compact::compress → 返回压缩后的诊断
- **特点**: 使用 lspz-core 的 compact format 压缩诊断，减少 token 消耗

### get_completions

- **输入**: `{ uri, backend, language, line, character }`
- **流程**: didOpen → send_request(textDocument/completion) → 返回补全列表

### get_symbols

- **输入**: `{ uri, backend, language }`
- **流程**: didOpen → send_request(textDocument/documentSymbol) → 返回符号列表

## 使用方式

```bash
# 启动 MCP server（stdio 模式）
lspz mcp

# Claude Desktop 配置
# {
#   "mcpServers": {
#     "lspz": {
#       "command": "lspz",
#       "args": ["mcp"]
#     }
#   }
# }
```

## 实现记录

- `lspz-mcp` crate：src/{lib,server,session,pool}.rs
- CLI 入口：`crates/lspz/src/main.rs` — Cli enum 增加 `Mcp` 子命令
- 错误处理：统一使用 `rmcp::ErrorData` 类型
- 连接生命周期：LspSession 在 McpServer 生命周期内保持，通过 Arc<Mutex<>> 共享
