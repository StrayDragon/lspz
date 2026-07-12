# Proposal: fix-completion-cap-pending-prune

## Why

CompletionCompressor 硬编码截断 50，与 Config.capping 双轨；`pending_requests` 在取消/无响应时永不清理会泄漏。

## What Changes

- `max_items=0` 表示不截断；默认 0；MCP 直调同样传 0
- `$/cancelRequest` 移除 pending；TTL 剪枝
- `just verify` + `scripts/verify-all.sh` 一键全量验证

## Capabilities

- `interceptors`, `proxy`

## 非目标

- 改 CappingInterceptor 语义

## Ethics

- `ethics.risk_level`: low
- `ethics.required_evidence`: `just verify` 通过
