# Proposal: fix-document-sync-unification

## Why

`LspSession::open_or_update_document` 已是正确的文档同步实现，但 `AgentHandle` 与 `DaemonMcpServer` 各自手写 `didOpen`/`version`，导致重复打开、版本冲突与错误诊断归属。

## What Changes

- AgentHandle 改用 `open_or_update_document`；移除独立 `doc_versions`
- 新增 daemon `lsp/sync_document`，DaemonMcpServer 全部改走该 RPC
- `get_diagnostics` 按 URI 过滤
- `LspSession::close_document` 清理 open 状态

## Capabilities

- `mcp`, `agent-sdk`

## 非目标

- per-session 锁；文档刷新

## Ethics

- `ethics.risk_level`: medium
- `ethics.required_evidence`: session/agent/daemon 单测 + `just qa`
