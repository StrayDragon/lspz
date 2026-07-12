# Proposal: fix-diag-code-utf8-notify-buffer

## Why

审计遗留三项 medium 问题会损害压缩正确性与 MCP/Agent 诊断可靠性：数字型 `Diagnostic.code` 被丢弃、message 截断可 panic、`send_request` 期间通知被 trace 后丢弃。

## What Changes

- 统一提取 `code`（string | number → String）写入 compact `c`
- UTF-8 安全截断 `normalize_message`
- `LspSession` 缓冲交错通知，供后续 `wait_for_notification*` 消费

## Capabilities

- `compression`, `mcp`

## 非目标

- MarkupContent completion docs；daemon status 计数器

## Ethics

- `ethics.risk_level`: medium
- `ethics.required_evidence`: 单元测试 + `just qa`
