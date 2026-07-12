# Proposal: fix-mcp-accept-empty-diagnostics

## Why

Daemon 路径已把首个（可空）`publishDiagnostics` 当作终态；in-process `McpServer` 仍等待非空，干净文件固定烧满 ~10s。

## What Changes

- 对齐 daemon：首个匹配 URI 的诊断通知即返回（含空）

## Capabilities

- `mcp`

## 非目标

- 不改 Agent SDK 诊断等待策略（除非顺带统一）

## Ethics

- `ethics.risk_level`: low
- `ethics.required_evidence`: 干净文件 get_diagnostics <1s；`just qa`
