# AGENTS.md

Project conventions for lspz (LSP compression proxy).

> **SSOT (Single Source of Truth)**: 本文档是 lspz 项目的通用规范文档。所有开发指南、文档都应引用本文档，避免重复和不一致。

---

## Rust

### Edition & Toolchain

- Edition 2024
- Stable toolchain (pinned in `rust-toolchain.toml`)
- All cargo commands use `--workspace` flag (workspace split planned per PRD)

### Code Quality

- Clippy thresholds configured in `.clippy.toml`
- Clippy thresholds configured in `.clippy.toml`; lint flags in justfile
- Format config in `rustfmt.toml`
- Nightly-only options are not allowed

### 命名约定

| 类型 | 约定 | 示例 |
|------|------|------|
| Struct | `PascalCase` | `Proxy`, `Config` |
| Enum | `PascalCase` | `Direction`, `Severity` |
| Function | `snake_case` | `get_diagnostics`, `compress` |
| Const | `SCREAMING_SNAKE_CASE` | `MAX_BUFFER_SIZE` |
| Trait | `PascalCase` | `Transport`, `Interceptor` |

### 错误处理

- 使用 `Result<T, LspzError>` 统一错误类型
- 使用 `thiserror` 定义错误枚举
- 避免使用 `.unwrap()` 和 `.expect()`（除测试代码）

### 异步代码

- 使用 `tokio` 作为异步运行时
- 使用 `async-trait` 定义异步 trait
- 所有 I/O 操作基于异步

### 日志规范

- 使用 `tracing` 库（非 `log`）
- 日志级别：ERROR, WARN, INFO, DEBUG, TRACE
- 结构化日志，使用 `info!`, `debug!`, `error!`, `warn!`

---

## Scripts (`scripts/`)

- **Purpose**: 只用于复杂的、项目特定的逻辑
- **Naming**: kebab-case
- **避免**: 简单的 cargo 命令包装（直接在 justfile 中调用）
- **No underscores** in filenames — prevents accidental Python module import

### 适合 scripts/ 的场景

- ✅ 复杂的验证逻辑（多步骤、条件判断）
- ✅ 需要与其他工具集成的脚本
- ✅ 项目特定的构建/部署逻辑

### 不适合 scripts/ 的场景

- ❌ 简单的 `cargo fmt`, `cargo clippy`, `cargo test` 等
- ❌ 只是转发命令的包装器（如 `check-rust-clippy.py`）

### 示例

**好的脚本** (`check-conventional-commit.py`):
- 包含复杂的 Git 提交消息解析和验证逻辑
- 项目特定的提交规范

**不必要的脚本** (`check-rust-clippy.py`, `check-rust-fmt.py`, `check-rust-test.py`):
- ❌ 已删除，直接在 justfile 中调用 cargo 命令

---

## Git Hooks (`prek.toml`)

- Only `repo = "builtin"` + `repo = "local"` — zero network dependency at runtime
- Local hooks: `pass_filenames = false` + `require_serial = true` (avoid cargo concurrent conflicts)
- Stages:
  - **pre-commit**: cargo-fmt, cargo-clippy (+ builtin whitespace/toml/yaml checks)
  - **commit-msg**: conventional commits validation
  - **pre-push**: cargo-test (full tests, not on every commit)

---

## Commits

- Conventional Commits format: `type(scope)!: description`
- Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert
- Subject line max 72 characters
- Validated by `scripts/check-conventional-commit.py`

---

## Task Runner (`justfile`)

- `just setup` = 安装 prek hooks + 构建
- `just fmt` = 格式化代码（写入）
- `just fmt-check` = 检查格式（只读）
- `just lint` = 运行 clippy（直接调用 cargo）
- `just test` = 运行测试（直接调用 cargo）
- `just qa` = fmt-check + lint + test（所有检查）
- `just check` = qa 的别名
- `just ci` = prek run --all-files + qa（完整 CI 模拟）
- Shell: `bash -euo pipefail`

- Shell: `bash -euo pipefail`

**原则**: Scripts 是 SSOT，justfile 和 prek hooks 委托给脚本执行。

---

## 文档规范

### Markdown

- 使用 [Mermaid](https://mermaid.js.org/) 绘制图表
- 代码块指定语言：\`\`\`rust, \`\`\`bash
- 链接使用相对路径（内部文档）

### 代码注释

- 公共 API 必须有文档注释（`///` 或 `//!`）
- 模块级文档使用 `//!`
- 包含使用示例（可运行）

### 文档优先级

1. **AGENTS.md** (本文件) - 通用规范 SSOT
2. **docs/specs/** - 技术规格
3. **docs/plan/** - 阶段计划
4. **docs/guides/** - 开发指南（引用 AGENTS.md，不重复）

### 文档生成规则 (SSOT)

> **详细规则**: 参见 [docs/specs/004-ssot-rules.md](docs/specs/004-ssot-rules.md)

#### 生成物标记

- **全文件生成**: 任何包含 `.gen.` 的文件都是自动生成的，禁止手工编辑
  - 例如: `modules.gen.md`, `config.gen.md`
  - 修改方式: 编辑 SSOT 源文件，运行 `just gen-*` 命令

- **注入区块**: 使用 AUTOGEN 注释标记的区块
  ```markdown
  <!-- BEGIN AUTOGEN:<id> -->
  (自动生成内容，不手工编辑)
  <!-- END AUTOGEN:<id> -->
  ```

#### SSOT 位置

| 内容类型 | SSOT 位置 | 生成物 |
|---------|----------|--------|
| API 文档 | 代码中的 `///` / `//!` 注释 | `docs/api/*.gen.md` |
| 模块索引 | `src/lib.rs` 等 crate 入口 | `docs/api/modules.gen.md` |
| 使用示例 | `examples/*.rs` 可运行代码 | `docs/guides/examples.gen.md` |
| 配置参考 | `src/config.rs` 结构体定义 | `docs/reference/config.gen.md` |
| 错误类型 | `src/error.rs` 错误定义 | `docs/reference/error-types.gen.md` |
| 项目元数据 | `Cargo.toml` | README.md 中的版本信息 |

#### 生成命令 (MVP 后可用)

```bash
just gen-docs      # 生成所有文档
just gen-api-docs  # 生成 API 文档
just gen-check     # 检查文档是否过时 (CI 使用)
```

#### 漂移检测

- 所有生成脚本必须支持 `--check` 模式（只检查不写入）
- `just qa` 包含文档漂移检测
- CI 中运行 `just gen-check` 确保文档与代码同步

---

## 测试规范

### 测试组织

- 单元测试：与源码同目录的 `tests` 模块
- 集成测试：`tests/` 目录
- 性能测试：`benches/` 目录（使用 criterion）

### 测试命名

- 测试函数：`test_<功能>_<场景>`
- 测试模块：`tests` 或 `<module>_tests`

### 覆盖率目标

| 层级 | 目标覆盖率 |
|------|-----------|
| 核心逻辑 (codec/) | ≥ 90% |
| Proxy 核心 | ≥ 80% |
| Transport | ≥ 70% |
| 配置/错误 | ≥ 60% |

---

---

## 异步超时与 CI 可靠性

### 教训: 无限循环必须有超时保护

2026-05-12: CI 在 `cargo test --workspace` 步骤卡死 1h+，原因是 `LspSession::send_request()`
和 `LspTestHarness::get_diagnostics()` 中的 `loop { transport.receive()... }` 没有超时保护。
当 LSP 服务器启动缓慢或版本不兼容时，这些循环永远不退出。

### 强制规则

| 模式 | 规则 | 示例 |
|------|------|------|
| 异步 receive 循环 | 必须使用 `tokio::time::timeout` 包裹 | `tokio::time::timeout(Duration::from_secs(30), self.transport.receive()).await?` |
| 测试 LSP 服务器 | 必须优雅跳过而非 panic | `if initialize failed → SKIP` |
| CI job | 必须设置 `timeout-minutes` | `timeout-minutes: 15` (job 级别) |
| CI step | 每个耗时步骤单独设置超时 | `timeout-minutes: 10` (test 步骤) |

### 超时值参考

| 场景 | 建议超时 | 说明 |
|------|---------|------|
| LSP 请求/响应 | 30s | 足够 LSP 服务器返回结果 |
| E2E 测试 (单个) | 30–60s | 含 LSP 服务器启动 + 分析 |
| CI test 步骤 | 10 min | 完整测试套件 |
| CI job 总超时 | 15 min | 全流程 (fmt + lint + test + gen-check) |
| CI clippy 步骤 | 5 min | 纯静态检查 |
| CI fmt 步骤 | 3 min | 纯静态检查 |
| CI gen-docs | 2 min | 脚本执行 |

### E2E 测试最佳实践

1. **外部服务检测**: 使用 `which` 检查二进制是否存在，不存在 → 优雅跳过
2. **启动失败处理**: 服务器启动或初始化握手失败 → 优雅跳过，不 panic
3. **断言宽容**: 不假设外部服务一定返回特定结果；验证"通知已收到"而非"诊断非空"
4. **项目脚手架**: 为每个语言创建最小项目结构（`Cargo.toml`、`go.mod` 等），否则 LSP 服务器无法分析文件
5. **启动参数**: 不同服务器需要不同参数（如 `typescript-language-server --stdio`），通过 `try_new_with_args()` 支持

### 错误示例 vs 正确示例

```rust
// ❌ 错误: 无限循环，无超时
loop {
    let raw = self.transport.receive().await?;
    // 如果服务器无响应，永远不退出
}

// ✅ 正确: 带超时的 receive 循环
loop {
    let raw = tokio::time::timeout(
        Duration::from_secs(30),
        self.transport.receive()
    )
    .await
    .map_err(|_| "timeout waiting for response")??;
    // 30s 后优雅返回错误
}
```

```rust
// ❌ 错误: 外部服务失败导致测试 panic
let result = server.initialize().await.unwrap();

// ✅ 正确: 外部服务失败时优雅跳过
if harness.initialize().await.is_err() {
    eprintln!("  SKIP: server init failed");
    return None;
}
```

### CI 费用控制

GitHub Actions 按运行时间计费。必须为每个 job 和耗时 step 设置合理的 `timeout-minutes`：

```yaml
jobs:
  qa:
    timeout-minutes: 15    # 最高安全阀
    steps:
      - name: Run tests
        timeout-minutes: 10  # 每个步骤独立超时
        run: cargo test --workspace
```

> 原则: 宁可超时失败（CI 红色），不可让 job 无限运行（费用爆炸）。

---

## SSOT Harness (代码即 SSOT)

### 原则

**代码是唯一的真相源 (Single Source of Truth)**。所有衍生文档从代码注释中自动生成，禁止手写 `.gen.` 文件。

### 生成规则

| SSOT 位置 | 生成物 | 生成命令 |
|-----------|--------|---------|
| `///` / `//!` 注释 | `docs/api/*.gen.md` | `just gen-api-docs` |
| `pub trait Interceptor` 定义 + 实现者 | `docs/specs/interceptors.gen.md` | `just gen-api-docs` |
| `pub struct Config` 字段 + 文档 | `docs/reference/config.gen.md` | `just gen-config-docs` |
| `pub enum LspzError` 变体 | `docs/reference/error-types.gen.md` | `just gen-error-docs` |
| `Cargo.toml` 工作区成员 + 依赖 | README.md 版本区块 | `just gen-meta-docs` |

### 架构不变量 (Architecture Invariants)

以下规则在代码实现中必须遵守，任何偏离需在 AGENTS.md 和设计评审中记录：

1. **Interceptor chain 是核心抽象**: 所有 Server→Client 消息转换必须通过 `Interceptor` trait，禁止在 Proxy 核心中直接硬编码消息处理逻辑
2. **Fail-open**: 任何压缩/转换失败 → 只记录 WARN 日志，透明转发原始消息，禁止抛出异常或中断 LSP 通信
3. **Transport-agnostic core**: `lspz-core` 不依赖任何特定传输实现（stdio/TCP/WS），只依赖 `Transport` trait
4. **Config-driven**: 所有运行时行为通过 `Config` 结构体控制，禁止硬编码开关或行为
5. **Zero-copy preference**: 在 Hot Path（消息收发、拦截器处理）中优先使用引用 `&str` / `&[u8]`、`Cow`、`Arc`，避免不必要的克隆
6. **LSP version locked**: 明确锁定 LSP 3.17 规范，不引入 3.18+ 特性直到项目正式声明支持

### 文档漂移检测

`just qa` 包含以下检查：

1. `just gen-check` → 检查 `.gen.` 文件是否与当前源码同步（修改时间、内容哈希）
2. `.gen.` 文件如果有手工编辑痕迹（git diff 检测）则 CI 失败
3. 缺少文档注释 → 全局 `#![warn(missing_docs)]` 或 clippy 规则
4. 架构不变量 → 代码审查中人工核对

### 架构级代码注释规范

公共 API 必须包含模块级（`//!`）注释，格式如下：

```rust
//! # 模块名
//!
//! 一句话说明模块职责。
//!
//! ## 核心概念
//!
//! - **概念 A**: 解释
//! - **概念 B**: 解释
//!
//! ## 架构图
//!
//! [MermaidChart:./docs/mmd/module-name.mmd]
//!
//! ## 注意事项
//!
//! - 扩展点预留说明
//! - 线程安全说明
//! - 错误处理说明
```

公共结构体、trait、枚举必须包含 `///` 文档注释，包含：

- 一句话说明（必须）
- 使用示例（复杂类型必须）
- 通过 `[`TypeName`]` 引用关联类型

## 依赖管理

### Cargo.toml

- Workspace 成员统一版本
- 依赖版本在 workspace dependencies 中定义
- 避免循环依赖

### 外部依赖

| 依赖 | 用途 |
|------|------|
| tokio | 异步运行时 |
| serde | 序列化框架 |
| serde_json | JSON 支持 |
| async-trait | 异步 trait |
| thiserror | 错误定义 |
| tracing | 日志/追踪 |
| rstest | 参数化测试（开发依赖） |

---

## 安全考虑

### 输入验证

- 验证外部输入（配置、命令参数）
- 避免未检查的索引访问（使用 `.get()`）
- 命令注入防护（验证 backend_cmd）

### 错误处理

- 压缩失败降级到透明转发
- 传输错误记录日志，不 panic
- 配置错误早期检测

---

## 性能考虑

### 避免不必要的克隆

- 优先使用引用 (`&str`, `&[u8]`)
- 使用 `Cow<T>` 避免条件分配
- 大对象使用 `Arc` 共享

### 异步最佳实践

- 避免 `.await` 在循环中（如可能）
- 使用 `tokio::spawn!` 并行独立任务
- 合理设置超时

---

## 版本管理

### 语义化版本

- v0.1.0: MVP (Library + Proxy)
- v0.2.0: MCP 集成
- v0.3.0: Agent SDK

### 兼容性承诺

- lspz-core 公共 API: 主版本不变时向后兼容
- 压缩格式: Agent 端解压库支持所有历史版本
- LSP 兼容性: 永不破坏标准 LSP 协议

---

---

## 检查点规则 (Checkpoint Save)

### 原则

每个阶段或重要模块完成后，必须进行验证 → 提交 → 版本号更新 → 打 tag，以此建立可追溯的检查点链。

### 执行流程

1. **验证**: 运行 `just qa` (lint + test + gen-check) 确保无错误
2. **提交**: 按照 Conventional Commits 格式提交，消息中注明当前阶段/模块
3. **版本号更新**: 在 Cargo.toml 中递增版本号（语义化版本）
4. **打 tag**: 创建对应版本的 git tag: `git tag v{版本号}`

### 检查点示例

| 里程碑 | 版本 | 说明 |
|--------|------|------|
| 项目初始化 | v0.0.1 | crate name locking |
| Codec 层完成 | v0.1.0-alpha.1 | JSON-RPC 编解码 |
| Transport 层完成 | v0.1.0-alpha.2 | StdioTransport |
| Proxy Core 完成 | v0.1.0-beta.1 | LSP 握手 + 路由 |
| 诊断压缩完成 | v0.1.0-rc.1 | MVP 功能冻结 |
| MVP 发布 | v0.1.0 | 正式版 |

### 历史版本管理

- `git tag` 列表展示所有已发布的检查点
- `git diff v0.1.0-alpha.1..v0.1.0-alpha.2` 查看阶段间变更
- 禁止在打 tag 后修改历史（如有紧急修复，递增 patch 版本）

## 参考资源

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [The Rust Style Guide](https://rust-lang.github.io/style-guide/)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [LSP 3.17 Specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
