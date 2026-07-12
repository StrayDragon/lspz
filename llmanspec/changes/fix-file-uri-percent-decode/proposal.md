# Proposal: fix-file-uri-percent-decode

## Why
`strip_prefix("file://")` 后直接 `read_to_string`，`%20` 等编码路径失败。

## What Changes
- 共享 `path_from_file_uri`：百分号解码 + 合理 host 剥离
- MCP / daemon MCP / AgentHandle 统一使用

## Capabilities
- `mcp`, `agent-sdk`

## 非目标
- 完整 URL 解析库依赖（可用最小解码）

## Ethics
- `ethics.risk_level`: low
- `ethics.required_evidence`: 单测 %20；`just qa`
