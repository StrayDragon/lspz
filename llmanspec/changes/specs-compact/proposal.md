---
depends_on: []
---

## Why

llman-sdd 0.5.0 迁移后，11 个 capability 共 204 条规则全部为裸规则（0 enforced、0 嵌套场景），
`llman-sdd review` pending 基线 204，可执行场景为 0。规格以「描述性裸规则 + 同 `@req:id` 的
GWT 单行规则」重复表述，维护成本高且无法被 review 的 enforced 信号计量。

## What Changes

- 转场景（81）：每条描述性规则吸收其同 id GWT 单行规则，转为嵌套 `场景:`（假如/当/那么），
  场景名沿用原 GWT 规则名；规则标题与散文 MUST 原文不变。
- 合并（5，跨能力语义等价，`dedupe-req-ids` 无撞号故无换号）：
  - product-requirements r48 去重合并 → compression r16（场景 `dedup-with-code` 迁入）
  - interceptors r36 失败开放 → compression r5
  - lsp-compatibility r53 错误隔离 → compression r5（场景 `compression-fail` 迁入）
  - proxy r63 Cancel-safe 消息循环 → lsp-compatibility r60（场景 `cancel-safe-loop` 迁入）
  - lsp-compatibility r67 按 id 匹配握手响应 → proxy r69（"禁止把下一条任意消息当响应"
    并入场景 `handshake-id-match` 的 Then）
- 移除（2，真重复被替代）：interceptors `chain-skip-disabled`（≡r45 `config-toggle`）、
  product-requirements `lsp-compatible`（空断言，被 lsp-compatibility/proxy/transport 具体
  场景替代）。
- 保留（1）：product-requirements r62 LSP 兼容性（产品级双向兼容总则，无法程序化表达，
  具体行为由 lsp-compatibility r18/r37/r46/r60、proxy、transport 场景覆盖）。

预期终态：规则 204 → 82，pending 204 → 1（仅 r62），嵌套场景约 113。

## 非目标

- 不新增、不修改、不删除任何 MUST/SHALL 行为语义（仅表述结构压缩与语义等价合并）。
- 不改动任何 Rust 源码、不 bump Cargo.toml 版本（先例：225240c specs 迁移无版本变更）。
- 不执行 archive freeze（归档仅 372K / 21 个 change，dry-run 评审后判定无需冻结）。
- 不引入 req id 换号映射（无撞号）。

## 对现有架构的影响

仅影响 `llmanspec/specs/**` 文档层。AGENTS.md 架构不变量（Interceptor chain、Fail-open、
Transport-agnostic、Config-driven、Zero-copy、LSP 3.17 locked）的规格承载不变：fail-open
仍由 compression r5（scope 含 src/interceptors/、src/codec/）承载，cancel-safe 分帧仍由
lsp-compatibility r60 承载，握手 id 匹配仍由 proxy r69 承载。不涉及公共 API，无兼容性影响。
