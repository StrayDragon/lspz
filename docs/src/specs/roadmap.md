# lspz Project Roadmap

> **lsp** **z**ip — 对 AI Coding Agent 极其友好的 LSP 压缩代理

## 项目愿景

构建一个**三模态 LSP 压缩代理系统**，通过 Token 敏感的智能压缩，让 AI Coding Agent
用更少上下文理解更多代码问题。

---

## 三种产品形态

```
                    ┌──────────────────────┐
                    │   lspz (单一 crate)   │
                    │  Feature flags 编译   │
                    └───────────┬──────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
        ▼                       ▼                       ▼
┌───────────────────┐  ┌───────────────────┐  ┌───────────────────┐
│  Library Mode     │  │   Proxy Mode      │  │   MCP Mode        │
│  (no-default)     │  │   (default=cli)   │  │   (feature mcp)   │
└───────────────────┘  └───────────────────┘  └───────────────────┘
```

| 模式 | 目标用户 | 典型场景 | 集成方式 |
|------|----------|----------|----------|
| **Library** | 自研 Agent CLI | 完全控制，零开销 | `lspz::Proxy` / crate 的直接引用 |
| **Proxy** | Claude Code/Continue/Cody | 即插即用，透明代理 | `lspz --backend gopls` |
| **MCP** | 快速实验/多工具协同 | 融入生态，按需查询 | MCP server 配置 |

---

## 功能特性总览

### 8 个拦截器 (7 压缩 + 1 截断)

| 拦截器 | 覆盖方法 | 压缩策略 | Token 节省 |
|--------|----------|----------|-----------|
| `DiagnosticsCompressor` | `textDocument/publishDiagnostics` | 去重 + delta range + 枚举缩减 | 60–80% |
| `CompletionCompressor` | `textDocument/completion` | Kind 编码 + doc 去重 + 字段裁剪 | 15–20% |
| `HoverCompressor` | `textDocument/hover` | Markdown 折叠 + MarkupKind 缩减 | 24–79% |
| `DocumentSymbolCompressor` | `textDocument/documentSymbol` | SymbolKind 编码 + 递归 children | 33–72% |
| `LocationCompressor` | `textDocument/references` `definition` `implementation` `typeDefinition` | URI 去重 + delta range | 60–85% |
| `WorkspaceSymbolCompressor` | `workspace/symbol` | SymbolKind 复用 + URI 去重 | 70–76% |
| `WorkspaceDiagnosticCompressor` | `workspace/diagnostic` | unchanged 跳过 + 复用 DiagnosticsCompressor | 83–90% |
| `CappingInterceptor` | diagnostics / completions / symbols | 截断到 N 条后压缩 | 80–95% |

### 3 种输出格式

| 格式 | 说明 | 用途 |
|------|------|------|
| `toon` (默认) | Token-Oriented Object Notation，自解释行协议 + 表格 | LLM 消费最佳选择 |
| `json` | Compact JSON，缩写字段名 | 兼容模式 |
| `passthrough` | 标准 LSP JSON 透传 | 调试 / 不压缩 |

### 3 种传输层

| 传输 | Feature Flag | 说明 |
|------|-------------|------|
| `StdioTransport` | always | 子进程 stdio（默认） |
| `TcpTransport` | `transport-tcp` (default) | TCP socket 连接 |
| `WsTransport` | `transport-websocket` | WebSocket 连接 |

### 运行时能力

| 能力 | CLI flag | 说明 |
|------|----------|------|
| 响应截断 | `--max-diags` `--max-completions` `--max-symbols` | 压缩前截断大返回 |
| 压缩开关 | `--compress-xxx` (7 个独立开关) | 按需启用/禁用各压缩器 |
| 运行指标 | `--metrics` `--metrics-interval` | 压缩率、延迟、处理计数 |
| 配置热重载 | `--config <file>` | TOML 文件变更时自动 reload |
| 环境变量 | `LSPZ_*` | 所有 CLI flag 均有对应 env var |

### Feature Flags

| Feature | 说明 | 默认启用 |
|---------|------|---------|
| `cli` | CLI 入口 (`lspz proxy` 子命令) | ✅ |
| `mcp` | MCP 服务器 (`lspz mcp`, `rmcp` 集成) | ❌ |
| `agent-sdk` | Agent SDK API (自动启用 `mcp`) | ❌ |
| `transport-tcp` | TCP socket 传输 | ❌ |
| `transport-websocket` | WebSocket 传输 | ❌ |

### Agent SDK 查询方法

| 方法 | LSP 方法 | 压缩支持 |
|------|----------|---------|
| `get_diagnostics` | `textDocument/publishDiagnostics` | ✅ |
| `get_completions` | `textDocument/completion` | ✅ |
| `get_symbols` | `textDocument/documentSymbol` | ✅ |
| `get_hover` | `textDocument/hover` | ✅ |
| `get_references` | `textDocument/references` | ✅ |
| `get_definition` | `textDocument/definition` | ✅ |
| `get_implementation` | `textDocument/implementation` | ✅ |
| `get_type_definition` | `textDocument/typeDefinition` | ✅ |
| `get_workspace_symbols` | `workspace/symbol` | ✅ |
| `get_workspace_diagnostics` | `workspace/diagnostic` | ✅ |
| `rename` | `textDocument/rename` | ✅ |
| `code_action` | `textDocument/codeAction` | ✅ |
| `formatting` | `textDocument/formatting` | ✅ |
| `notify_change` | `textDocument/didChange` | — |
| `notify_close` | `textDocument/didClose` | — |
| `notify_save` | `textDocument/didSave` | — |
| `send_raw` | 任意方法 | — |
| `compress` / `inflate` | — | 手动压缩/解压 |

### MCP Tools (lspz mcp)

| Tool | 说明 |
|------|------|
| `get_diagnostics` | 获取文件诊断（含压缩） |
| `get_completions` | 获取光标位置补全 |
| `get_symbols` | 获取文档符号 |

---

## 版本历史

| 版本 | 里程碑 |
|------|--------|
| **v0.11.2** *(当前)* | Daemon 会话复用、空闲回收、socket 规范化 |
| v0.11.1 | Daemon 协议去同步修复、孤儿响应清理 |
| v0.9.2 | 文件同步、workspace root、重构操作、Pool 委托重构 |
| v0.9.0 | Location 压缩, TCP/WebSocket, 运行指标, 配置热重载 |
| v0.8.0 | Response Capping, Proxy 响应拦截 |
| v0.7.0 | TOON 输出格式, Benchmark 报告系统 |
| v0.6.0 | DocumentSymbol 压缩 |
| v0.5.0 | Hover + DocumentSymbol 压缩 |
| v0.4.0 | 生产加固, 补全压缩 |
| v0.3.0 | Agent SDK (AgentHandle + AgentPool) |
| v0.2.0 | MCP 集成 (3 tools) |
| v0.1.0 | MVP (Proxy + 诊断压缩) |

---

## 已排除的 LSP 方法

以下方法经评估不适合压缩：

| 方法 | 排除原因 |
|------|----------|
| `textDocument/codeAction` | 交互性操作，token 量不大，ROI 低 |
| `textDocument/semanticTokens/full` | 响应已是 delta 压缩后的扁平数组 |
| `textDocument/signatureHelp` | 通常只有 1-5 个 signature，数据量极小 |
| `textDocument/inlayHint/codeLens/documentHighlight` | UI 辅助功能，AI agent 使用频率低 |

---

## 已排除的技术方向

| 方向 | 排除原因 |
|------|----------|
| `#[derive(LspInterceptor)]` proc macro | 8 个拦截器手写成本 < proc-macro crate 维护成本 |
| Python/TypeScript 解压客户端库 | 压缩格式仍在演进，等待稳定后发布 |
| `rstest` 参数化测试 | 当前测试模式已足够，无强需求 |

---

## 快速验证

```bash
just compress-demo   # 演示所有 7 个压缩器的 token 节省
just qa              # fmt + clippy + test + gen-check
just gen-docs        # 从代码注释重新生成 .gen.md 文档
just bench           # Criterion 吞吐量基准
```

---

## 技术栈

- **Rust** 2024 edition
- **Tokio** 异步运行时
- **Serde** + **serde_json** 序列化
- **Clap** CLI 参数解析
- **rmcp** MCP 协议 SDK
- **notify** 文件系统监听（配置热重载）

---

## 文档导航

### 设计规格 (SSOT)
- [../architecture.md](../architecture.md) — 架构设计
- [compression-format.md](./compression-format.md) — 压缩格式
- [lsp-compatibility.md](./lsp-compatibility.md) — LSP 兼容性
- [ssot-rules.md](./ssot-rules.md) — SSOT 规则
- [toon-format.md](./toon-format.md) — TOON 格式

### 开发指南
- [../../../AGENTS.md](../../../AGENTS.md) — 项目规范 SSOT
- [../guides/agent-integration.md](../guides/agent-integration.md) — Agent SDK 集成
- [../guides/claude-desktop.md](../guides/claude-desktop.md) — Claude Desktop 配置

### 参考
- [../api-reference.md](../api-reference.md) — API 文档
- [../benchmarks.md](../benchmarks.md) — 压缩基准报告

---

## 许可证

MIT
