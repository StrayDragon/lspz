# Proposal: fix-daemon-graceful-shutdown

## Why

Daemon 在 SIGINT、idle reaper 与 `daemon/shutdown` 路径调用 `std::process::exit(0)`，跳过 `Drop`，导致 `StdioTransport` 无法 `kill` LSP 子进程；`daemon/shutdown` 还错误依赖 `LSPZ_SOCKET`，常留下残留 socket。

## What Changes

- 用共享 shutdown 信号打断 accept 循环，退出前 `clear` 会话池并删除真实 `socket_path`
- `daemon/shutdown` 使用服务器持有的路径，不再依赖环境变量
- （可选加固）`LspPool::shutdown` / `clear` 显式 drop 会话

## Capabilities

- `mcp` — daemon 生命周期与会话池回收

## Impact

- 行为：关闭后不再残留 rust-analyzer/gopls 等孤儿进程与 stale socket
- 兼容：对外协议不变

## 非目标

- per-session 锁（另 change）
- 文档同步统一（另 change）

## Ethics

- `ethics.risk_level`: medium
- `ethics.prohibited_actions`: 不在未回收子进程时 `process::exit`
- `ethics.required_evidence`: `just qa` + 针对 shutdown 信号路径的单测/逻辑覆盖
