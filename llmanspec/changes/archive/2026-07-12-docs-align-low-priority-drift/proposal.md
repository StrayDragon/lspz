# Proposal: docs-align-low-priority-drift

## Why
低优先级漂移：`OutputFormat::Json` 注释仍写「当前默认值」（实际默认 `toon`）；`llmanspec/config.yaml` 与 `deploy-docs.yml` 仍引用已删除的 mdbook/`docs/src`。

## What Changes
- 修正 Json 变体文档注释
- 更新 llmanspec context 文档描述
- deploy-docs 仅发布 cargo doc

## Capabilities
- `ssot-rules`

## 非目标
- 恢复 mdbook；改运行时默认值

## Ethics
- `ethics.risk_level`: low
- `ethics.required_evidence`: `just verify`
