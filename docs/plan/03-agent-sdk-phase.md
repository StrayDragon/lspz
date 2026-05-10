# lspz Agent SDK 阶段计划 (v0.3)

**阶段**: Phase 3 - Agent SDK
**版本**: v0.3.0
**状态**: ✅ 已完成
**目标周期**: 2026-05
**实际完成**: 2026-05-10

## 目标

提供 `lspz-agent-sdk` crate，让 AI Agent CLI 开发者能以声明式方式嵌入 LSP 能力，
无需手动管理 LSP 进程生命周期和消息编排。

## 架构

```
┌──────────────────────────────────────────┐
│             AI Agent CLI                  │
│    cargo add lspz-agent-sdk              │
└──────────────────┬───────────────────────┘
                   │
┌──────────────────▼───────────────────────┐
│         lspz-agent-sdk                    │
│                                           │
│  ┌─────────────────────────────────────┐  │
│  │        AgentHandle                   │  │
│  │  - Builder: backend + language       │  │
│  │  - get_diagnostics()                 │  │
│  │  - get_completions()                 │  │
│  │  - get_symbols()                     │  │
│  │  - inflate() / compress()            │  │
│  │  - shutdown()                        │  │
│  └────────────────┬────────────────────┘  │
└───────────────────┬───────────────────────┘
                    │ delegates to
┌───────────────────▼───────────────────────┐
│              lspz-mcp :: LspSession        │
│  spawn → initialize → send_request        │
└───────────────────┬───────────────────────┘
                    │
┌───────────────────▼───────────────────────┐
│            lspz-core :: compact            │
│  compress / decompress                    │
└───────────────────────────────────────────┘
```

## API 设计

```rust
// 一行启动
let mut agent = AgentHandle::builder()
    .backend("rust-analyzer")
    .language("rust")
    .enable_compression(true)
    .start()
    .await?;

// 按需查询
let diags = agent.get_diagnostics("file:///src/main.rs").await?;

// 解压（如果开启了 compression）
let expanded = agent.inflate(&diags)?;

// 关闭
agent.shutdown().await?;
```

## 核心文件

| 文件 | 说明 |
|------|------|
| `crates/lspz-agent-sdk/Cargo.toml` | crate 配置 |
| `crates/lspz-agent-sdk/src/lib.rs` | 模块入口 |
| `crates/lspz-agent-sdk/src/agent.rs` | AgentHandle + AgentBuilder |

## 依赖关系

- `lspz-core` — compact format codec（compress/decompress）
- `lspz-mcp` — LspSession（进程管理、消息收发）

## 已完成

- [x] crate 骨架（Cargo.toml, lib.rs）
- [x] AgentHandle + AgentBuilder
  - [x] start（spawn + initialize）
  - [x] get_diagnostics（可选压缩）
  - [x] get_completions
  - [x] get_symbols
  - [x] inflate / compress（改为关联函数，无需 &self）
  - [x] shutdown
- [x] LspSession 重构: Box&lt;dyn Transport&gt;（支持 mock 注入）
- [x] MockTransport（lspz-core::transport::mock）
- [x] AgentPool + AgentPoolBuilder（多语言 LSP 会话管理）
- [x] 单元测试 16 个（AgentHandle 11 + AgentPool 5）
- [x] Agent 集成指南（docs/guides/agent-integration.md）
- [x] Claude Desktop 集成指南（docs/guides/claude-desktop-integration.md）
- [x] CI/CD（GitHub Actions）
- [x] 许可证: MIT

## 待办

- [x] 单元测试（mock LSP server）— MockTransport + 11 AgentHandle + 5 AgentPool 测试
- [x] 多 language/连接池支持（LspPool）— AgentPool 实现
- [ ] Proc macros（`#[derive(LspInterceptor)]` 等）— 延迟（仅 1 个 Interceptor）
- [ ] Skill 生成（从 LSP capabilities → LLM tool descriptions）— 延迟（设计待定）
