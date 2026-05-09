# _HANDOFF.md — lspz v0.1.0 MVP

> 创建: 2026-05-09
> 状态: **Task D 进行中** (测试/文档/发布)

## 已完成 (Tasks A, B, C)

| Task | Tag | 说明 |
|------|-----|------|
| A | `v0.1.0-alpha.1` | Workspace + JSON-RPC codec + Transport (30 tests) |
| B | `v0.1.0-alpha.2` | Proxy state machine + CLI (30 tests) |
| C | `v0.1.0-rc.1` | 诊断压缩管道 + compact format (51 tests) |

### 最近 5 个 commits (最新在前)

```
19c19c6 fix: gen-docs trailing blank line drift with prek
33098b5 feat: implement diagnostic compression pipeline and compact format
abd4001 feat: implement proxy state machine and CLI entry point
d429fcf feat: implement workspace structure, JSON-RPC codec, and transport layer
99e5ca8 docs: add algorithm mermaid diagrams in Chinese
```

## 当前状态

- `just qa` 全通过 (51 tests, 0 clippy warnings, prek 全绿)
- integration test 文件已创建但 **未完全编译通过**
- 核心代码稳定，可启动: `lspz --backend rust-analyzer`

## 待完成 (Task D)

### D1: Integration tests (⌛ 阻塞中)

**文件位置**: `lspz-core/tests/compression_integration.rs`

**问题**: 添加了 `tiktoken = "3.1"` 作为 dev-dependency, 但网络问题导致下载卡住 (国内网络访问 crates.io 可能超时)。测试文件里有 7 个 test:

1. `test_gopls_compression_roundtrip` — 13 条 gopls 诊断 → 压缩 → 解压验证
2. `test_gopls_dedup_with_normalization` — 归一化后 13→4 组验证
3. `test_rust_analyzer_compression_roundtrip` — RA 诊断压缩验证
4. `test_rust_analyzer_dedup` — RA 3→2 组验证
5. `test_token_savings_gopls` — Token 节省 (预期 ≥50% basic, ≥70% norm)
6. `test_token_savings_rust_analyzer` — Token 节省
7. `test_normalization_increases_dedup` — 归一化提高去重率验证

**备选方案**: 如果 tiktoken 下载问题无法解决, 可以:
- 删除 `[dev-dependencies]` 中的 tiktoken, 用 JSON 字节长度作为 token 数的近似
- 或使用 `tiktoken-rs` (0.11) 替代 (也是 wrapper)
- 或等网络恢复后重试

### D2: SSOT doc generation

✅ 已通过 (`just gen-docs` + `just gen-check` 全绿)

### D3: Release v0.1.0

- 创建 `CHANGELOG.md`
- `git tag v0.1.0`
- 可选: 发布 crates.io

## 关键架构不变式 (不可违反)

1. Interceptor chain 是唯一的 Server→Client 消息转换入口
2. Fail-open: 任何压缩失败 → WARN 日志 + 透明转发原始消息
3. lspz-core 不依赖任何特定传输实现
4. 所有运行时行为通过 Config 控制
5. SSOT: 代码注释是唯一真相源, .gen.md 都是生成物

## 开发命令

```bash
just qa          # fmt + lint + test + gen-check
just gen-docs    # 从代码注释重新生成 .gen.md
just gen-check   # 检查 .gen.md 是否与代码同步
cargo test --test compression_integration  # 运行集成测试
```

## 继续开发的 Prompt

以下 prompt 可直接给下一个 Agent:

---

```
请继续 lspz v0.1.0 MVP 的 Task D (Integration tests, docs, release)。

## 当前状态

Task A/B/C 已完成并打 tag。代码在 commit 19c19c6, 51 个 unit test 全通过。

## 待办列表

### 1. 修复集成测试的 tiktoken 依赖问题

- `lspz-core/Cargo.toml` 中有 `[dev-dependencies] tiktoken = "3.1"`
- 如果网络下载失败, 可以改用纯 Rust 实现或移除 token 计数测试
- 先尝试 `cargo test --test compression_integration --no-run` 看编译是否通过
- 如果 tiktoken 3.x API 变化导致编译错误, 请搜索最新 API 并适配 (可能是 `cl100k_base().unwrap()` 这种方式)
- 备选: 删除 tiktoken 依赖, 用 JSON 字节长度做近似

### 2. 运行集成测试

```bash
cargo test --test compression_integration -- --nocapture
```

验证:
- gopls 13 条诊断 → 4 组 (9 unused vars → 1, 2 unused imports → 1, 1 type mismatch, 1 unused param)
- rust-analyzer 3 条 → 2 组
- Token 节省 ≥50% (basic) / ≥70% (with normalization)

### 3. 创建 CHANGELOG.md

在项目根目录创建, 记录 v0.1.0-alpha.1, v0.1.0-alpha.2, v0.1.0-rc.1, v0.1.0

### 4. 最终检查

```bash
just qa           # 必须全通过
cargo test --test compression_integration  # 集成测试全通过
```

### 5. Tag v0.1.0

```bash
git tag v0.1.0
```

## 重要提示

- gen-docs 脚本在 `scripts/gen-docs.py`, 修改后需保证 `just gen-check` 通过
- 不要修改核心压缩逻辑 (diagnostics.rs / compact.rs) — Task C 已完成
- prek hook 会修改 trailing blank lines, gen-docs.py 有 `.rstrip("\n") + "\n"` 处理
```
