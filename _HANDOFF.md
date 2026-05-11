# _HANDOFF.md — lspz v0.4.0 (completed)

> 创建: 2026-05-09
> 更新: 2026-05-11
> 状态: **v0.5.0 已完成** — Hover 压缩

## 已完成 (Phase 0–4)

| Phase | Tag | 说明 |
|-------|-----|------|
| 0 | — | 项目初始化、SSOT 规则、CI/CD、许可证 |
| 1 | `v0.1.0` | MVP: Library + Proxy 模式, JSON-RPC codec, 诊断压缩 |
| 2 | `v0.2.0` | MCP 集成: McpServer + LspPool + 3 tools |
| 3 | `v0.3.0` | Agent SDK: AgentHandle + AgentPool + 单元测试 |
| 4 | `v0.4.0` | 生产加固 & 补全压缩 |
| 5 | `v0.5.0` | HoverCompressor: 消息压缩 + 6 测试 |

## Phase 4 交付物

| 项目 | 状态 | 说明 |
|------|------|------|
| InterceptorChain fail-open bug | ✅ | original params 在 take() 前保存 |
| 规范化: basedpyright + TypeScript | ✅ | 新增 reportUnusedVariable/Import + 数值 code |
| Gen-docs 枚举正则修复 | ✅ | 跳过 #[error/from] 行 |
| Mermaid 架构图 ×6 | ✅ | 含 completion-compression.mmd |
| README 更新 v0.3.0 | ✅ | 版本 + Agent SDK 标记 |
| Git tags v0.1.0/v0.3.0 | ✅ | 补打缺失标签 |
| 清理 stale 目录 | ✅ | target/package/lspz-0.0.1 |
| E2E 测试框架 | ✅ | LspTestHarness + 4 语言 fixtures |
| CompletionCompressor | ✅ | 字段裁剪 + kind 枚举缩减 + doc 去重 |

## 最新测试统计

```
cargo test --workspace:   ~90+ passed (lib + integration + e2e)
cargo clippy:             零警告
cargo fmt --check:        通过
gen-check:                15/15 文件同步
```

## 当前版本号

| Crate | 版本 |
|-------|------|
| `lspz-core` | 0.1.0 |
| `lspz-mcp` | 0.2.0 |
| `lspz-agent-sdk` | 0.3.0 |
| `lspz` (CLI) | 0.2.0 |

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
v0.4.0 → 生产加固 & 补全压缩
```

## 版本标签

```
v0.1.0 → MVP (Library + Proxy)
v0.2.0 → MCP 集成 (McpServer + LspPool + 3 tools)
v0.3.0 → Agent SDK (AgentHandle + AgentPool)
v0.4.0 → 生产加固 & 补全压缩
v0.5.0 → Hover 压缩
```

## 已延迟 (待 Phase 6+)

参见 `_HANDOFF_PHASE5.md` 获取完整上下文。
