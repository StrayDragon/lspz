# Proposal: fix-proxy-idle-receive-timeout

## Why

`message_loop` 对 server `receive()` 套了与握手相同的 30s `timeout`。编辑器空闲超过 30s 且无服务器推送时，Proxy 会以 `Timeout` 退出，破坏长期代理会话。

## What Changes

- 空闲 `select!` 中的 server receive 不再使用致命 idle timeout
- 保留握手/请求等待路径上的超时保护

## Capabilities

- `proxy`

## 非目标

- 不改 handshake 超时；不引入心跳协议

## Impact

- 行为修复：空闲代理可长期存活

## Ethics

- `ethics.risk_level`: medium
- `ethics.required_evidence`: unit/integration 证明 idle >30s 不退出；`just qa`
