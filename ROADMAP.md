# lspz Project Roadmap

> **lsp** **z**ip — 对 AI Coding Agent 极其友好的 LSP 压缩代理

## 项目愿景

构建一个**三模态 LSP 压缩代理系统**，通过 Token 敏感的智能压缩，让 AI Coding Agent
用更少上下文理解更多代码问题。

### 核心价值

- **Token 节省**: 4 种 LSP 消息类型压缩，平均 40–80%
- **透明集成**: Agent 无需感知，像正常使用 LSP 一样
- **灵活部署**: 支持作为库、独立代理、MCP 服务器三种形态
- **标准兼容**: 永不破坏 LSP 协议标准（fail-open 保证）

---

## 三种产品形态

```
                    ┌─────────────────┐
                    │    lspz-core    │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│Library Mode  │    │ Proxy Mode   │    │  MCP Mode    │
│  #1 优先     │    │  #2 优先     │    │  #3 优先     │
└──────────────┘    └──────────────┘    └──────────────┘
```

| 模式 | 目标用户 | 典型场景 | 集成方式 |
|------|----------|----------|----------|
| **Library** | 自研 Agent CLI | 完全控制，零开销 | `use lspz_core::Proxy` |
| **Proxy** | Claude Code/Continue/Cody | 即插即用，透明代理 | `lspz --backend gopls` |
| **MCP** | 快速实验/多工具协同 | 融入生态，按需查询 | MCP server 配置 |

---

## 版本历史

| 版本 | 阶段 | 主要变更 |
|------|------|----------|
| **v0.6.0** *(当前)* | 4 个压缩器全部完成 | DocumentSymbol 压缩 |
| v0.5.0 | Hover 压缩 | Markdown 紧凑 + Hover 字段压缩 |
| v0.4.0 | 补全压缩 | CompletionItemKind 编码 + doc 去重 |
| v0.3.0 | Agent SDK | AgentHandle + AgentPool |
| v0.2.0 | MCP 集成 | 3 tools (get_diagnostics/completions/symbols) |
| v0.1.0 | MVP | Proxy + Diagnostics 压缩 |

---

## 已完成 (Phase 0–6)

### Phase 0: 基础设施 ✅
- [x] 项目初始化
- [x] CI/CD 配置 (GitHub Actions)
- [x] SSOT 规则建立
- [x] 开发环境搭建

### Phase 1: MVP — v0.1 ✅
- [x] JSON-RPC codec + Transport
- [x] Proxy 核心 + CLI
- [x] Interceptor 链 + Diagnostics 压缩
- [x] 测试 + 文档 + 发布

### Phase 2: MCP 集成 — v0.2 ✅
- [x] lspz-mcp crate
- [x] get_diagnostics / get_completions / get_symbols tools
- [x] MCP 配置指南

### Phase 3: Agent SDK — v0.3 ✅
- [x] lspz-agent-sdk crate (AgentHandle + AgentPool)
- [x] Agent 集成模板

### Phase 4: Completion 压缩 — v0.4 ✅
- [x] CompletionItemKind 枚举缩减 (1-25 → 单 char)
- [x] 字段裁剪 + doc 去重
- [x] E2E 测试框架 (LspTestHarness)

### Phase 5: Hover 压缩 — v0.5 ✅
- [x] Markdown 空白行折叠 + code fence 缩短
- [x] MarkupKind 缩减 (markdown → "m", plaintext → "p")
- [x] Hover 字段压缩

### Phase 6: DocumentSymbol 压缩 — v0.5 ✅
- [x] DocumentSymbol (分层) + SymbolInformation (扁平) 双格式支持
- [x] SymbolKind 1-26 单字符编码
- [x] 递归 children 压缩

---

## 剩余事项 (低优先级)

### Metrics & Tracing 增强
埋点记录压缩率 / 延迟 / 节省 token 数。不违反 fail-open 原则。

### TCP/WebSocket Transport
当前仅支持 stdio。无实际需求，除非需要远程 LSP server。

### Config 热重载
`notify` crate + `Arc<RwLock<Config>>`。但有状态一致性问题，重启即可。

### Python/TypeScript 客户端库
等待压缩格式稳定（连续 3 个 phase 无变更）。

---

## 技术栈

- **Rust**: 2024 edition
- **Tokio**: 异步运行时
- **Serde**: 序列化框架
- **Clap**: CLI 参数解析

---

## 文档导航

### 新手入门
1. [README.md](README.md) — 项目概览和快速开始
2. [docs/README.md](docs/README.md) — 开发者前导
3. [docs/specs/001-tri-modal-architecture.md](docs/specs/001-tri-modal-architecture.md) — 架构规格

### 技术规格
- [docs/specs/002-compression-format.md](docs/specs/002-compression-format.md) — 压缩格式规范
- [docs/specs/003-lsp-compatibility.md](docs/specs/003-lsp-compatibility.md) — LSP 兼容性
- [docs/specs/004-ssot-rules.md](docs/specs/004-ssot-rules.md) — SSOT 规则

### API 文档（自动生成）
- [docs/api/modules.gen.md](docs/api/modules.gen.md) — 模块索引
- [docs/api/config.gen.md](docs/api/config.gen.md) — 配置参考
- [docs/reference/config.gen.md](docs/reference/config.gen.md) — 配置字段说明
- [docs/specs/interceptors.gen.md](docs/specs/interceptors.gen.md) — 拦截器列表

---

## 许可证

MIT
