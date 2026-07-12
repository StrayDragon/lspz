# Proposal: refactor-pool-per-session-locks

## Why

Daemon 与 in-process MCP 在 `pool.lock()` 期间执行长达数十秒的 LSP I/O，全局串行化所有会话。

## What Changes

- `LspPool` 以 `Arc<Mutex<LspSession>>` 存会话
- 查找/创建只短暂持有池锁；LSP await 仅持有会话锁

## Capabilities

- `mcp`

## 非目标

- AgentPool 重构；文档

## Ethics

- `ethics.risk_level`: medium
- `ethics.required_evidence`: `just qa`
