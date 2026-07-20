# Proposal: add-mcp-workspace-roots

## Why

Cursor 等编码 Agent 经 MCP 调用 `get_diagnostics` / `get_completions` / `get_symbols` 时，往往已通过 **MCP Roots** 声明工作区，却仍被迫传完整 `file://` URI。缺少 roots/`peer_info` 绑定后，无参或空 URI 调用无法落到正确 workspace，Agent 体验差、易误用进程 cwd。

## What Changes

- MCP 连接后捕获并使用真实 **MCP Roots** 与 `peer_info` / `client_info`（至少支持 Cursor 类客户端）
- 工具参数：`uri` 变为可选；无参 / 空 URI 时按 **Roots → 进程 cwd / `detect_workspace_root`** 解析工作区
- 默认无 URI 流：解析工作区 → 扫描/触发相关 LSP → 返回 **max-column / compact TOON** 摘要（面向 Agent 消费）
- 显式 `file://` 仍可用；在已知 Roots 时，路径解析优先落在工作区内（相关 QA：无沙箱可读任意路径 → 本变更仅做 roots-aware 约束，不做全量安全重写）

## Capabilities

- `mcp`（主）
- 间接触及 `codec`（TOON 摘要形态；行为合约以 mcp delta 为准）

## Impact

- **架构**：MCP handler 增加 session-scoped workspace 上下文（roots + peer）；工具 schema / 输入类型变更。主线已落地 **rmcp 2.2** + Roots/`peer_info` 接线（本变更继续承载行为合约与剩余测试/文档任务）
- **兼容性**：显式 URI 调用保持可用；无 URI 为新增默认路径。工具 JSON Schema 去掉 `uri` required（破坏性对「强制必填」客户端可忽略；Cursor 类受益）
- **与 QA 关系**：纳入「`file://` 应相对 workspace」的相关部分；**daemon 认证 / socket TOCTOU / `owns_daemon` / 压缩双轨 SSOT** 明确 Out of Scope（见设计）

## 非目标

- 不在本 change 内重复升级 `rmcp`（已由主线完成 1.x→2.2）
- 不实现 daemon 认证、socket `chmod`/`SO_PEERCRED`、TOCTOU 单飞、backend allowlist
- 不把 MCP 路径全面改走 interceptor chain / 压缩 SSOT 统一（可后续变更）
- 不改 Agent SDK 公共 API（除非为共享 workspace 解析抽公共函数）

## Ethics

- `ethics.risk_level`: medium
- `ethics.prohibited_actions`: 不得在无用户确认时扩大为全量 daemon 安全重写；不得静默读取 Roots 外敏感路径作为默认无参扫描目标
- `ethics.required_evidence`: Cursor 类（或模拟 Roots）无 URI 调用返回 compact TOON；显式 URI 回归；`llman sdd validate add-mcp-workspace-roots --strict`；实现阶段 `just qa`
- `ethics.refusal_contract`: 若要求跳过 Roots 直接任意路径 RCE/读盘，拒绝并指向显式 URI + 既有 fail-open 行为
- `ethics.escalation_policy`: Roots 多根选择策略、无参扫描范围（全仓 vs 启发式）有歧义时升级用户确认
