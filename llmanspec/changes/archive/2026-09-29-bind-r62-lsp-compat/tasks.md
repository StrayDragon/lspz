# Tasks: bind-r62-lsp-compat

- [x] 1. 落地 r62 嵌套场景：编辑 `llmanspec/specs/product-requirements/product-requirements.feature`，向 `@req:r62` 规则块插入 `e2e-handshake` / `passthrough-unmodified` / `response-id-preserved` 三个场景（步骤映射既有测试，见 proposal），并 commit（specs-landed）
  - 测试 seam：复用既有 harness——`tests/e2e_lsp_servers.rs`（真实服务器 e2e，外部服务缺失时优雅跳过）与 `src/proxy.rs` tests 模块（passthrough / toon id 单测）；不发明新 harness。场景本身由 `specs.check_command` 守护，无新增覆盖率要求（维持现测试覆盖）
- [x] 2. 验证：`llman-sdd validate bind-r62-lsp-compat --strict` 全绿；`llman-sdd spec unbound --limit 0` total 1 → 0；在 change 分支上跑 `cargo test --workspace --all-features` 全绿（对照 merge-base，确认未动 `target/package/` 下任何文件）
