# Design: bind-r62-lsp-compat

## 决策 1：场景落在 product-requirements，不落在 lsp-compatibility

`spec unbound` 指向的是产品视图 r62（capability=product-requirements）。机制级条款
（输入兼容 r18、透明转发 r37、能力协商 r46）已绑定在 `lsp-compatibility.feature`，
r62 作为产品级 MUST 需要自己的可执行锚点，而非把场景挂到机制 spec 上敷衍计数。

## 决策 2：三个场景全部映射既有测试，零实现改动

| 场景 | 守护测试 | 层 |
|------|---------|---|
| `e2e-handshake` | `tests/e2e_lsp_servers.rs`（rust-analyzer/gopls 等 initialize 握手 + 诊断通知，服务器缺失时优雅跳过） | e2e |
| `passthrough-unmodified` | `src/proxy.rs` `test_passthrough_skips_compression` | 单元 |
| `response-id-preserved` | `src/proxy.rs` `test_toon_response_preserves_id` | 单元 |

备选方案（否决）：为 r62 新写专门测试。否决理由——既有测试已完整覆盖「标准接口兼容」
的可判定面（握手、透明转发、id 保持），新测试只会重复同一 seam；场景的价值是给
既有守护一个规范锚点。

## 决策 3：不引入 Gherkin runner

`config.yaml` 已声明 `specs.check_command: cargo test --workspace --all-features`
且注明无 Gherkin runner。场景步骤以中文 GWT 描述行为，由该命令端到端守护；
场景文本本身不经步骤代码绑定执行。

## 架构影响

无。Interceptor 链、Transport、fail-open 语义均不动；本 change 只在绑定分支
`sdd/bind-r62-lsp-compat` 修改 `llmanspec/specs/product-requirements/product-requirements.feature`。
