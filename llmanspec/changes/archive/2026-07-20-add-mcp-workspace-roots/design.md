# Design: add-mcp-workspace-roots

## 背景

现状（`src/mcp/server.rs`）：工具 schema 将 `uri` 标为 `required`；工作区仅靠 `detect_workspace_root(uri)` 自文件路径向上找 marker。MCP 侧未消费客户端 **Roots** / `peer_info`，Cursor 等无法省略 URI。

主线并行工作：`rmcp` → 2.2 与 Roots 底层 API **已落地**（`Cargo.toml` `rmcp = "2"`，`src/mcp/workspace.rs`）。本设计定义行为合约；剩余实现任务见 `tasks.md`（测试 / 文档 / 门禁）。

## 决议

### 1. Session workspace 上下文

MCP 连接 / initialize 完成后：

1. 记录 `peer_info` / `client_info`（name、version 等，便于 Cursor 类探测与日志）
2. 若客户端声明 Roots 能力：请求并缓存 `roots` 列表（`file://` 根 URI）
3. Roots 变更通知（若协议支持）：更新缓存；失败则 WARN 并保留上次成功值（fail-open）

### 2. URI / 工作区解析优先级

对工具调用：

| 输入 | 行为 |
|------|------|
| 非空 `uri`（`file://` 或可规范化为本地路径） | 使用该 URI；若已缓存 Roots，**SHOULD** 校验落在某一 root 下（越界 → 明确错误，不静默读） |
| 缺省 / 空字符串 `uri` | 取第一个可用 MCP Root（多根时：优先与 `peer_info` 暗示一致，否则首个 root；歧义见 Ethics escalation） |
| 无 Roots | fallback：`std::env::current_dir()`，再对候选路径跑现有 `detect_workspace_root` 逻辑 |
| 皆失败 | 返回可行动的错误（提示提供 `uri` 或配置 Roots），不得 panic |

`get_completions` 在无 URI 时仍 **MUST** 要求 `line`/`character` *或* 提供明确错误（无参 completions 无意义）；本变更的「无参默认流」主要针对 **`get_diagnostics`（及可选 `get_symbols` 工作区摘要）**。

### 3. 默认无 URI 流（diagnostics）

```text
resolve_workspace()
  → 发现工作区内相关源文件 / 语言（启发式：marker + 常见扩展，有上限）
  → 经 LspPool get_or_spawn + open_or_update / 触发诊断
  → compact 压缩 + max-column / agent-oriented TOON 摘要返回
```

「max-column / compact TOON」：在现有 `codec::compact` + `codec::toon` 之上，为工作区级摘要增加列数/行数上限与聚合字段（例如按 severity 计数 + 截断后的 top-N 行），避免一次 dump 整仓诊断撑爆 Agent 上下文。具体上限可配置，默认保守。

### 4. 与 `_QA_REPORT.md` 的折叠

| Finding | 本变更 |
|---------|--------|
| `file://` 无 workspace 沙箱 | **部分纳入**：有 Roots 时越界拒绝；无 Roots 时保持现状并文档化风险 |
| Daemon 无认证 / socket TOCTOU / `owns_daemon` | **Out of Scope / Future** |
| `get_or_spawn` 竞态单飞 | Out of Scope（可另开 change） |
| MCP 压缩双轨 / 绕过 interceptor | Out of Scope；tasks 仅要求摘要走现有 compact→TOON，不强制 interceptor |

## 代码落点（实现阶段）

- `src/mcp/server.rs` / `daemon_server.rs`：schema、解析、无参流
- 新建或扩展 `src/mcp/workspace.rs`（可选）：roots 缓存 + resolve
- `src/codec/toon.rs`（或 mcp 侧包装）：workspace summary TOON
- 测试：`tests/` 或 `src/mcp/*` 单测模拟 Roots / 空 URI

## 依赖与风险

- **阻塞依赖**：rmcp 2.x Roots API — **已解除**（主线）。后续以 Cursor 模拟测试与 `just qa` 为合入门禁。
- **多根歧义**：需产品确认；MVP 取首个 root + WARN。
