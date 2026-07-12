# Proposal: fix-daemon-client-owned-shutdown

## Why
`owns_daemon=true` 的 Drop 只打日志，auto-start 的 daemon 会残留到 idle TTL。

## What Changes
- Drop 时 best-effort 新连接发送 daemon/shutdown

## Capabilities
- `mcp`

## 非目标
- 同步阻塞整个 runtime；强制 kill -9

## Ethics
- `ethics.risk_level`: medium
- `ethics.required_evidence`: 单测或可观测 shutdown 调用；`just qa`
