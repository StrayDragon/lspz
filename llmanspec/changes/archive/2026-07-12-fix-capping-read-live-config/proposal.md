# Proposal: fix-capping-read-live-config

## Why
Capping 上限在构建链时固化；热重载只更新 enable 开关，改数字无效。

## What Changes
- CappingInterceptor 持有 Arc<RwLock<Config>>，intercept 时读 live 上限

## Capabilities
- `interceptors`

## 非目标
- 热重载重建整条链

## Ethics
- `ethics.risk_level`: low
- `ethics.required_evidence`: 单测改 config 后 cap 变化；`just qa`
