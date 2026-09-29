---
depends_on: []
branch: sdd/bind-r62-lsp-compat
base_branch: main
base_sha: fc930bfc7eb4a763047f1af1d2cfa10590655426
---

# 绑定 r62 LSP 兼容性为可执行场景

## Why

`llman-sdd spec unbound` 报告 r62「LSP 兼容性」为裸规则（无嵌套场景），缺行为守护。该条款是产品级 MUST，机制级守护已存在（lsp-compatibility spec 的输入兼容 r18 / 透明转发 r37 / 能力协商 r46）且对应测试全绿，仅产品视图缺可执行锚点。

## What Changes

- 向 `llmanspec/specs/product-requirements/product-requirements.feature` 的 r62 规则块追加嵌套「场景:」，全部映射既有测试，不改任何实现代码：
  - `e2e-handshake`：真实 LSP 服务器经 lspz proxy 完成 initialize 握手（tests/e2e_lsp_servers.rs）
  - `passthrough-unmodified`：非目标消息透明转发不改写（src/proxy.rs `test_passthrough_skips_compression`）
  - `response-id-preserved`：压缩后响应 JSON-RPC id 不变（src/proxy.rs `test_toon_response_preserves_id`）

## 非目标

- 不新增/修改 `src/` 实现代码
- 不改 `lsp-compatibility` capability 既有条款
- 不引入 Gherkin runner；场景由 `cargo test --workspace --all-features`（specs.check_command）守护

## Capabilities

- `specs/product-requirements`（唯一改动点）

## 对现有架构的影响

无。纯 specs 补全；Interceptor 链、Transport、fail-open 等架构均不动，守护测试全部已存在。公共 API 无变更。
