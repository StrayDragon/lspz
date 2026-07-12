# Proposal: docs-refresh-stale-specs

## Why

`lsp-compatibility` 与 `product-requirements` 相对 `origin/main` 的 scope 代码已更新但 spec 未跟进，导致 `llman sdd validate --strict` STALE。同时 `mcp` r3 仍写 AgentPool，与实现 LspPool 漂移；product 的「无损」与去重键与 compression spec 不一致。

## What Changes

- 为 lsp-compatibility 补充 FrameState 取消安全分帧、握手按 id 匹配要求
- 对齐 product-requirements：Token 压缩语义、去重键含 code、显式 daemon 模式
- 将 mcp 会话管理要求改为 LspPool

## Capabilities

- `lsp-compatibility`, `product-requirements`, `mcp`

## 非目标

- 不改运行时行为（纯规格同步）
- 不修复 idle receive timeout 等剩余 medium bug（另提案）

## Impact

- 仅 llmanspec；strict validate 应恢复全绿

## Ethics

- `ethics.risk_level`: low
- `ethics.required_evidence`: `llman sdd validate --all --strict --no-interactive`
