# Proposal: add-mcp-workspace-path-binding

## Why

全局 MCP（`user-lspz`）在 Cursor 未提供 Roots 时会 fallback 到进程 cwd（常为 `$HOME`），无 `uri` 扫描会误读无关文件。Agent 常传项目相对路径（`paths`）但当前 schema 不支持且未知字段被静默丢弃。协议侧 Roots（SEP-2577）正弃用，官方迁移方向是工具参数显式传路径。

## What Changes

- `get_diagnostics` / `get_symbols` 支持 `path` / `paths` / `workspace`；相对路径相对 workspace 锚点解析
- 新增 `set_workspace`：会话级绑定 workspace，后续相对路径无感
- Workspace 锚点优先级：工具 `workspace` 参数 → 会话绑定 → MCP Roots → 可信 fallback（有项目 marker 且非 home/`/`）
- 不可信 fallback（home/`/`/无 marker）MUST 返回可行动错误，禁止扫描
- 工具入参 `deny_unknown_fields`：未知字段（如误用字段名）MUST 明确报错
- Roots 仍作 best-effort；增强日志以便诊断客户端未声明/失败原因

## Capabilities

- `mcp`（主）

## Impact

- **架构**：MCP server 增加会话级 `session_workspace`；`workspace.rs` 统一路径解析与安全阀
- **兼容性**：显式绝对 `file://` uri 保持可用；无参扫描仅在可信 workspace 下继续
- **协议**：对齐 SEP-2577「路径经 tool parameters」；Roots 保留为可选增强

## 非目标

- 不实现新的 MCP 协议扩展或 elicitation/input_required Roots 流（draft 2026-07-28）
- 不改 Agent SDK / Proxy / interceptor 压缩路径
- 不做全量文件系统沙箱（仅扫描与相对路径解析的锚点约束）

## Ethics

- `ethics.risk_level`: medium
- `ethics.prohibited_actions`: 不得在不可信 fallback（如 `$HOME`）上静默扫描；不得静默忽略未知工具参数
- `ethics.required_evidence`: 单测覆盖相对 paths、set_workspace、home reject、unknown field reject；`llman sdd validate`；`just qa`
