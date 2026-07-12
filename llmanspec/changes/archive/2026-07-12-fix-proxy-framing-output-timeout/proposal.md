# Proposal: fix-proxy-framing-output-timeout

## Why

审计发现 Proxy 热路径存在帧取消不安全、输出格式未按 Config 生效、握手/关闭未按 `id` 匹配响应、以及分帧无大小上限与 receive 无超时等问题，会在真实 LSP 流量下导致挂死、协议错位或配置无效。

## What Changes

- 将 stdin / server 分帧读改为 cancel-safe（专用 reader task + channel），避免 `select!` 中途取消丢字节
- 正确实现 `OutputFormat::Passthrough`（跳过压缩，原样转发）
- 默认/配置为 TOON 时，对 **response** 也输出 TOON，且保留 JSON-RPC `id`
- 握手与 shutdown 按请求 `id` 匹配响应，期间转发服务器通知；receive 加超时
- 统一 Content-Length 上限（与 codec `MAX_BODY_SIZE` 对齐）；修复 `blocking_read` 在 async 上下文的用法
- WebSocket 接受 Text 帧（按 UTF-8 字节作为 LSP 帧）

## Capabilities

- `proxy` — 消息循环、握手、输出格式、超时
- `transport` — 分帧安全与大小上限、WebSocket Text
- `codec` — Passthrough / TOON response 形状（与 proxy 协作）

## Impact

- 公共行为：`passthrough` 与 `toon` 对 response 的输出形状变化（符合既有文档/spec 意图）
- 非破坏：`json` 模式行为保持 compact JSON response
- 测试：新增/更新 proxy、framing、output-format 单测

## 非目标

- Daemon 优雅退出与子进程回收（另 change）
- 文档同步 `open_or_update` 统一（另 change）
- 池级 per-session 锁（另 change）
- AGENTS/README 文档刷新（另 change）

## Ethics

- `ethics.risk_level`: medium（协议与 I/O 正确性）
- `ethics.prohibited_actions`: 不跳过测试、不禁用 fail-open
- `ethics.required_evidence`: `just qa` 通过 + 针对 framing/timeout/output 的单测
