# Phase 5 Handoff — Remaining Deferred Items

> 创建: 2026-05-11
> 更新: 2026-05-11
> 前提: v0.5.0 已完成 (`_HANDOFF.md` 含完整 Phase 0-5 总结)

## 项目状态摘要

```
Phase 0-4: ✅ 全部完成
Phase 5:   ✅ HoverCompressor (v0.5.0)
git tags:  v0.1.0, v0.2.0, v0.3.0, v0.4.0, v0.5.0
crates:    lspz-core 0.1.0 / lspz-mcp 0.2.0 / lspz-agent-sdk 0.3.0 / lspz (CLI) 0.2.0
Tests:     ~90+ passed (lib + integration + e2e skeleton)
Diagrams:  7 Mermaid files in docs/mmd/
```

## 已完成

### ✅ HoverCompressor (v0.5.0)

| 文件 | 说明 |
|------|------|
| `crates/lspz-core/src/interceptors/hover.rs` | 拦截器实现 + 6 单元测试 |
| `crates/lspz-core/src/interceptors/mod.rs` | `pub mod hover;` |
| `crates/lspz-core/src/lib.rs` | `pub use HoverCompressor;` |
| `crates/lspz-core/src/config.rs` | `enable_hover_compress` + builder + env-var |
| `crates/lspz/src/main.rs` | `--compress-hover` / `-H` flag + chain 注册 |
| `docs/mmd/hover-compression.mmd` | pipeline flowchart |

**QA**: fmt + clippy + 90 tests + gen-check 全部通过

## 剩余待实施项

### 1. DocumentSymbol 压缩 — 低优先级

**原因**: token 量比 completions 低, AI agent 使用频率低

**实现提示**: 类似 CompletionCompressor 模式:
- `DocumentSymbol` 是树状结构 (children 嵌套)
- 需要递归压缩
- Compact field names: `name`→`n`, `kind`→`k`, `range`→`r`, `children`→`c`, `detail`→`d`
- `SymbolKind` (1-26) 可以枚举缩减, 参考 `encode_completion_kind()` 模式
- Drop: `deprecated`, `tags`, `selectionRange`

**需要创建**: `crates/lspz-core/src/interceptors/symbols.rs`
**需要修改**: 同 HoverCompressor 的 5 个文件

---

## 传输层 (低优先级 — 无外部需求)

### 3. TCP/WebSocket Transport

**原因**: 当前所有使用场景都是 stdio (本地进程). 无需求.

**如果未来需要**:
- 在 `crates/lspz-core/src/transport/` 下添加 `tcp.rs` 和 `websocket.rs`
- 实现 `Transport` trait (`send` + `receive` async)
- 参考 `transport/stdio.rs` 的实现模式

---

## 可观测性 (中优先级 — 按需实施)

### 4. Metrics & Tracing 增强

**当前状态**: 只有基础 `tracing` (info/warn/error/trace)

**建议**:
- 添加 `tracing-metrics` 可选 feature
- 关键埋点: 压缩前/后 token 数, 处理延迟, 压缩率
- 注意不要违反 fail-open 原则 — metrics 收集失败不应影响消息转发

---

## 配置 (低优先级 — 复杂且有风险)

### 5. Config 热重载

**依赖**: `notify` crate + `Arc<Config>` swap

**风险**: Config 中包含 `enable_*_compress` 开关 — 热重载期间如果 interceptor chain 正在处理消息, 可能导致状态不一致

**建议方案**:
```rust
// proxy.rs
struct Proxy {
    config: Arc<RwLock<Config>>,
    // interceptor_chain 在每次 process 时从 config 重建
    // 或: interceptor_chain 本身在 RwLock 中
}
```

**优先级**: 低 — 当前配置在启动时设置, 重启即可变更

---

## 解压缩库 (低优先级 — 等待格式稳定)

### 6. Python/TypeScript 客户端库

**背景**: lspz-core 的压缩格式 (`CompactDiagnostics`, `CompactCompletion` 等) 仍在演进. Phase 4 新增了 completion 压缩格式, 未来可能还有 hover/symbol.

**建议等待时机**: 当连续 3 个 phase 没有格式变更时, 锁定格式并发布客户端库.

**参考格式**:
- `CompactDiagnostics`: `{ version, uri, diagnostics: [{ m, s, r, c?, t?, n? }] }` — 见 `codec/compact.rs`
- `CompactCompletion`: `{ version, items: [{ l, k?, d?, doc?, doc_id?, i?, st?, dep?, te? }], docs?, incomplete }` — 见 `interceptors/completions.rs`

---

## 元编程 (低优先级 — 不值得)

### 7. `#[derive(LspInterceptor)]` proc macro

**现状**: 只有 3 个 Interceptor (DiagnosticsCompressor, CompletionCompressor, + 未来 HoverCompressor), 每个 ~50 行样板代码.

**不建议实施**: proc-macro crate 的维护成本 > 手写 3 个 Interceptor 的成本. 除非 Interceptor 数量达到 10+.

---

## 与 AI Agent SDK 集成指导

### 将 lspz 用作 AI 工具

`lspz-agent-sdk` crate 提供了 `AgentHandle` 和 `AgentPool`:

```rust
use lspz_agent_sdk::{AgentPool, AgentConfig};

let mut pool = AgentPool::new();
let handle = pool.get_or_spawn(
    "rust",
    AgentConfig::new("rust-analyzer")
        .with_max_completions(30)
        .enable_diag_compress(true)
        .enable_completion_compress(true),
).await?;

// 获取压缩后的诊断
let diags = handle.get_diagnostics("file:///src/main.rs").await?;

// 获取补全
let completions = handle.get_completions("file:///src/main.rs", 10, 4).await?;
```

**如果新消息类型 (hover/symbol) 添加了压缩**, 需要同时对 `lspz-agent-sdk` 做两件事:
1. 在 `AgentConfig` 中添加相应的 `enable_*_compress` 字段
2. 添加对应的方法 (如 `handle.get_hover(...)`)

---

## 架构模式回顾 (实现新 Interceptor 的步骤)

添加一个新的消息类型压缩 (如 HoverCompressor):

1. **创建拦截器文件**: `crates/lspz-core/src/interceptors/hover.rs`
   - 定义 struct + Default impl
   - 实现 `Interceptor` trait
   - 实现压缩逻辑
   - 添加单元测试 (至少 6 个)

2. **注册拦截器**:
   - `interceptors/mod.rs`: `pub mod hover;`
   - `lib.rs`: `pub use interceptors::hover::HoverCompressor;`

3. **配置支持**:
   - `config.rs`: 添加 `enable_hover_compress: bool`
   - `main.rs`: 在 CLI args 中添加 `--compress-hover` flag

4. **测试验证**:
   - `cargo test` — 单元测试通过
   - `just gen-docs && just gen-check` — 文档同步

---

## 验证命令

```bash
# 完整质量检查
just qa                    # fmt + clippy + test + gen-check

# 单个 crate
cargo test -p lspz-core    # 核心库测试 (不含 e2e)
cargo test -p lspz-core -- --ignored  # e2e 测试 (需要 LSP servers)

# 文档
just gen-docs && just gen-check
```

---

## 关键联系人

- 项目: lspz — LSP compression proxy for AI coding agents
- 仓库: `git@github.com:straydragon/lspz.git`
- 文档: `_HANDOFF.md` (Phase 0-4 完整总结), `docs/plan/` (各 phase 设计文档)
