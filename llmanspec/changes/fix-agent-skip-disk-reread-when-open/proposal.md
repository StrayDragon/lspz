# Proposal: fix-agent-skip-disk-reread-when-open

## Why

`open_file` 每次查询都 `read_to_string` + `open_or_update_document`，会在 `notify_change` 之后用磁盘内容覆盖 agent 未保存缓冲。

## What Changes

- 已打开 URI：查询路径跳过磁盘回读（或仅当未 open 时读盘）
- 保留显式 reopen/close 语义

## Capabilities

- `agent-sdk`

## 非目标

- MCP 路径 URI decode（另案）；不改 notify_change API

## Ethics

- `ethics.risk_level`: medium
- `ethics.required_evidence`: 单测 notify_change → get_* 不回读盘；`just qa`
