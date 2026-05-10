# lspz 项目路线图

> **lsp** zip - LSP 压缩代理：对 AI Coding Agent 极其友好的 LSP 代理层

## 项目愿景

构建一个**三模态 LSP 压缩代理系统**，通过 Token 敏感的智能压缩，让 AI Coding Agent 用更少上下文理解更多代码问题。

### 核心价值

- **Token 节省**: 诊断消息压缩 ≥40%，降低 API 成本
- **透明集成**: Agent 无需感知，像正常使用 LSP 一样
- **灵活部署**: 支持作为库、独立代理、MCP 服务器三种形态
- **标准兼容**: 永不破坏 LSP 协议标准

---

## 三模态架构

lspz 支持三种产品形态，满足不同使用场景：

```
                    ┌─────────────────┐
                    │    lspz-core    │
                    │   (纯逻辑库)     │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│Library Mode │    │ Proxy Mode   │    │  MCP Mode    │
│(作为库)     │    │(LSP 代理)     │    │(MCP 服务器)  │
│优先级: #1     │    │优先级: #2     │    │优先级: #3     │
└──────────────┘    └──────────────┘    └──────────────┘
```

| 模式 | 目标用户 | 典型场景 | 集成方式 |
|------|----------|----------|----------|
| **Library** | 自研 Agent CLI | 完全控制，零开销 | `use lspz_core::Proxy` |
| **Proxy** | Claude Code/Continue/Cody | 即插即用，透明代理 | `lspz --backend gopls` |
| **MCP** | 快速实验/多工具协同 | 融入生态，按需查询 | MCP server 配置 |

**详细架构说明**: [docs/specs/001-tri-modal-architecture.md](docs/specs/001-tri-modal-architecture.md)

---

## 开发路线

### Phase 0: 基础设施

**状态**: ✅ 已完成

- [x] 项目初始化
- [x] 文档结构建立
- [x] SSOT 规则建立 (docs/specs/004-ssot-rules.md)
- [ ] CI/CD 配置 (待办)
- [ ] 开发环境搭建指南 (待办)

### Phase 1: MVP - v0.1 (Library + Proxy)

**状态**: ✅ 已完成

**目标**: 实现核心压缩功能，支持作为库和代理两种模式

**实施计划**: 参见 [docs/plan/01-mvp-phase.md](docs/plan/01-mvp-phase.md) 中的 Task A→D 拆分

**检查点序列**:
- ✅ Task A (Codec + Transport) → `v0.1.0-alpha.1`
- ✅ Task B (Proxy Core + CLI) → `v0.1.0-alpha.2`
- ✅ Task C (Interceptor + 诊断压缩) → `v0.1.0-rc.1`
- ✅ Task D (测试 + 文档 + 发布) → `v0.1.0`

**详细计划**: [docs/plan/01-mvp-phase.md](docs/plan/01-mvp-phase.md)

---

### Phase 2: MCP 集成 - v0.2

**状态**: ✅ 已完成 | **实际完成**: 2026-05

**目标**: 添加 MCP 服务器模式，支持快速实验和生态集成

**核心功能**:
- [x] `lspz-mcp` crate
- [x] MCP tools 实现
  - [x] `get_diagnostics`
  - [x] `get_completions`
  - [x] `get_symbols`
- [ ] Claude Desktop 集成示例
- [ ] MCP 配置指南

**交付物**:
- [x] `lspz-mcp` crate (v0.2.0)
- [x] MCP 集成文档 (docs/plan/02-mcp-phase.md)

**详细计划**: [docs/plan/02-mcp-phase.md](docs/plan/02-mcp-phase.md)

---

### Phase 3: Agent SDK - v0.3

**状态**: ⏸️ 未开始 | **预计**: Q3 2026

**目标**: 提供简化的 Agent 集成 SDK，降低嵌入成本

**核心功能**:
- [ ] `lspz-macros` 宏库
- [ ] Agent 集成模板
- [ ] Skill/ Prompt 生成器
- [ ] 类型安全的 API

**交付物**:
- [ ] `lspz-macros` crate (v0.3.0)
- [ ] `lspz-agent-sdk` crate (v0.3.0)
- [ ] Agent 集成指南

**详细计划**: [docs/plan/03-agent-sdk-phase.md](docs/plan/03-agent-sdk-phase.md) (待创建)

---

### Phase 4: 高级特性 - v0.4

**状态**: ⏸️ 未开始 | **预计**: Q4 2026

**目标**: 扩展压缩范围，优化性能

**核心功能**:
- [ ] 更多消息类型压缩
  - [ ] Completion 压缩
  - [ ] Hover 压缩
  - [ ] DocumentSymbol 压缩
- [ ] TCP/WebSocket 传输层
- [ ] Metrics & Tracing
- [ ] 动态配置热加载

---

## 技术栈

### 核心依赖
- **Rust**: 2021 edition
- **Tokio**: 异步运行时
- **Serde**: 序列化框架
- **tower-lsp**: LSP 协议库（评估中）

### 测试目标 LSP 服务器
- rust-analyzer
- gopls
- basedpyright
- typescript-language-server

---

## 文档导航

### 新手入门
1. [docs/README.md](docs/README.md) - 开发者前导（必读！）
2. [docs/specs/001-tri-modal-architecture.md](docs/specs/001-tri-modal-architecture.md) - 架构规格
3. [docs/plan/01-mvp-phase.md](docs/plan/01-mvp-phase.md) - MVP 计划

### 技术规格
- [docs/specs/002-compression-format.md](docs/specs/002-compression-format.md) - 压缩格式规范
- [docs/specs/003-lsp-compatibility.md](docs/specs/003-lsp-compatibility.md) - LSP 兼容性
- [docs/specs/004-ssot-rules.md](docs/specs/004-ssot-rules.md) - 文档生成和 SSOT 规则

### 开发指南
- [docs/guides/coding-conventions.md](docs/guides/coding-conventions.md) - 编码约定
- [docs/guides/testing-guide.md](docs/guides/testing-guide.md) - 测试指南

---

## 版本历史

| 版本 | 日期 | 阶段 | 主要变更 |
|------|------|------|----------|
| v0.1.0 | 2025-12 | MVP | Library + Proxy 模式 |
| v0.2.0 | 2026-05 | MCP | 添加 MCP 服务器 (rmcp SDK, 3 tools) |
| v0.3.0 | TBD | SDK | Agent 集成 SDK |

---

## 贡献指南

请查看 [docs/guides/contributing.md](docs/guides/contributing.md) 了解如何参与贡献。

## 许可证

TBD
