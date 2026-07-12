# Proposal: fix-pool-spawn-without-global-await

## Why
`get_or_spawn` / `handle_spawn` 在持有全局 `LspPool` 锁时 await `initialize`，其他会话查找被阻塞数秒。

## What Changes
- 双检锁：池锁外 spawn+initialize，再短暂加锁插入
- `handle_spawn` 使用 `pool_key()` 与同一路径

## Capabilities
- `mcp`

## 非目标
- AgentPool；不改 idle reaper TTL

## Ethics
- `ethics.risk_level`: medium
- `ethics.required_evidence`: 单测/设计证明 initialize 不跨池锁；`just qa`
