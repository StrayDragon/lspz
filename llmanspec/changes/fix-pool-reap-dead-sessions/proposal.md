# Proposal: fix-pool-reap-dead-sessions

## Why
崩溃的 LSP 子进程仍留在 map 中，调用方持续协议错误直到 10min idle reaper。

## What Changes
- lookup/get_or_spawn 前 try_wait；已退出则 remove 再重建

## Capabilities
- `mcp`

## 非目标
- 主动 health check 定时器

## Ethics
- `ethics.risk_level`: medium
- `ethics.required_evidence`: 单测死会话被替换；`just qa`
