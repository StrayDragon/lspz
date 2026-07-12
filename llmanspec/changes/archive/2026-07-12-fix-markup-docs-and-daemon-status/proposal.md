# Proposal: fix-markup-docs-and-daemon-status

## Why

Completion 压缩只读 `documentation` 字符串，常见的 MarkupContent 对象被静默丢弃；DaemonStatus 的 `uptime_secs` / `total_connections` / `total_requests` 从未更新，导致 `daemon list/status` 恒为 0。

## What Changes

- `compress_completions` 提取 MarkupContent.value
- DaemonStatus 记录启动时间；accept 时 +connections；dispatch 时 +requests；status 时刷新 uptime

## Capabilities

- `interceptors`, `mcp`

## 非目标

- 新的 status 字段；改变 wire 协议形状

## Ethics

- `ethics.risk_level`: low
- `ethics.required_evidence`: 单测 + `just qa`
