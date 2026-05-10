# _HANDOFF.md — lspz v0.3.0 ✅ (Agent SDK)

> 创建: 2026-05-09
> 更新: 2026-05-10
> 状态: **v0.3.0 已发布** — Agent SDK 完整交付

## 已完成 (Phase 0–3)

| Phase | Tag | 说明 |
|-------|-----|------|
| 0 | — | 项目初始化、SSOT 规则、CI/CD、许可证 |
| 1 | `v0.1.0` | MVP: Library + Proxy 模式, JSON-RPC codec, 诊断压缩 |
| 2 | `v0.2.0` | MCP 集成: McpServer + LspPool + 3 tools |
| 3 | `v0.3.0` | Agent SDK: AgentHandle + AgentPool + 单元测试 |

## 最新测试统计

```
cargo test --workspace:  86 passed (57 unit + 13 integration + 16 agent-sdk)
cargo clippy:            零警告
cargo fmt --check:       通过
```

## 当前版本号

| Crate | 版本 |
|-------|------|
| `lspz-core` | 0.1.0 |
| `lspz-mcp` | 0.2.0 |
| `lspz-agent-sdk` | 0.3.0 |
| `lspz` (CLI) | 0.2.0 |

## Phase 3 交付物

- [x] `lspz-agent-sdk` crate (v0.3.0) — `AgentHandle` + `AgentPool`
- [x] 单元测试 (11 AgentHandle + 5 AgentPool) — 通过 MockTransport 实现 mock LSP
- [x] Agent 集成指南 — `docs/guides/agent-integration.md`
- [x] Claude Desktop 集成指南 — `docs/guides/claude-desktop-integration.md`
- [x] CHANGELOG 更新 (v0.2.0 + v0.3.0)
- [x] CI/CD (GitHub Actions: fmt + clippy + test + gen-check)
- [x] 许可证: MIT
- [x] git tag v0.3.0

## 已延迟 (待后续 Phase)

| 项目 | 原因 |
|------|------|
| `lspz-macros` proc macros (`#[derive(LspInterceptor)]`) | 仅 1 个 Interceptor, 不值得引入 proc-macro crate |
| Skill/Prompt 生成器 (LSP capabilities → LLM tool descriptions) | 设计待定 |
| Phase 4: Completion/Hover/DocumentSymbol 压缩 | 高级特性 |
| TCP/WebSocket 传输层 | 无需求 |

## 关键架构不变式 (不可违反)

1. Interceptor chain 是唯一的 Server→Client 消息转换入口
2. Fail-open: 任何压缩失败 → WARN 日志 + 透明转发原始消息
3. lspz-core 不依赖任何特定传输实现
4. 所有运行时行为通过 Config 控制
5. SSOT: 代码注释是唯一真相源, .gen.md 都是生成物

## 开发命令

```bash
just qa              # fmt + lint + test + gen-check
just gen-docs        # 从代码注释重新生成 .gen.md
just gen-check       # 检查 .gen.md 是否与代码同步
cargo test --workspace  # 全量测试
git tag -l 'v*'      # 查看所有版本 tag
```

## 版本标签

```
v0.1.0 → MVP (Library + Proxy)
v0.2.0 → MCP 集成 (McpServer + LspPool + 3 tools)
v0.3.0 → Agent SDK (AgentHandle + AgentPool)
```
