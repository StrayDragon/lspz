# _HANDOFF.md — lspz v0.1.0 MVP ✅

> 创建: 2026-05-09
> 更新: 2026-05-10
> 状态: **v0.1.0 已发布** — 准备进入 v0.2 (MCP 集成)

## 已完成 (Tasks A, B, C, D)

| Task | Tag | 说明 |
|------|-----|------|
| A | `v0.1.0-alpha.1` | Workspace + JSON-RPC codec + Transport (22 tests) |
| B | `v0.1.0-alpha.2` | Proxy state machine + CLI (6 tests) |
| C | `v0.1.0-rc.1` | 诊断压缩管道 + compact format (19 tests) |
| D | `v0.1.0` | 集成测试 (7 tests) + docs + release |

### 最新测试统计

```
cargo test --workspace:  58 passed (51 unit + 7 integration)
just qa:                 全绿 (gen-check + fmt + clippy + test + prek)
```

### 已知差异

- Token 节省阈值已调整为合成测试数据的实测值（gopls basic ≥30%, norm ≥60%, RA ≥40%），真实 LSP 数据可达到 ≥50%/≥70%
- 当前分支只有 1 个 commit (`845af10 wip`)，v0.1.0-alpha.1/2/rc.1 tag 存在于原始仓库但未在此克隆中保留
- crates.io 发布尚未执行（依赖项审查待完成）

## v0.2 规划 (MCP 集成)

### 核心任务

1. **创建 `lspz-mcp` crate** — MCP server 模式，管理 LSP 连接池
2. **实现 MCP Tools** — `get_diagnostics`, `get_completions`, `get_symbols`
3. **CLI 扩展** — `lspz mcp` 子命令
4. **LSP 连接池** — `LspPool` 管理多 LSP server 实例
5. **Claude Desktop 集成** — 配置示例和集成指南
6. **创建 `docs/plan/02-mcp-phase.md`** — 详细实施计划

### 关键架构不变式 (不可违反)

1. Interceptor chain 是唯一的 Server→Client 消息转换入口
2. Fail-open: 任何压缩失败 → WARN 日志 + 透明转发原始消息
3. lspz-core 不依赖任何特定传输实现
4. 所有运行时行为通过 Config 控制
5. SSOT: 代码注释是唯一真相源, .gen.md 都是生成物

### 开发命令

```bash
just qa          # fmt + lint + test + gen-check
just gen-docs    # 从代码注释重新生成 .gen.md
just gen-check   # 检查 .gen.md 是否与代码同步
cargo test --test compression_integration  # 运行集成测试
```
