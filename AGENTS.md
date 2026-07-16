<!-- LLMANSPEC:START -->
# LLMAN 规范驱动开发

本项目使用 llman SDD。阅读 `llmanspec/config.yaml` 了解 SDD 命令行为配置，以及 `llmanspec/AGENTS.md` 获取项目附加规则。

## 项目上下文

Tech stack: Rust (Edition 2024), Tokio async runtime
项目: lspz - AI-friendly LSP compression proxy
核心功能: 拦截 LSP 服务器响应，压缩成紧凑格式（TOON/compact），节省 AI token
三种用法: 库（嵌入 Rust agent）、CLI 代理、MCP 服务器
代码规范:
  - Conventional Commits 格式
  - Clippy 严格模式（配置在 .clippy.toml）
  - 使用 tracing 进行结构化日志
  - 错误处理: Result<T, LspzError> + thiserror
  - 异步代码: Tokio + async-trait
架构原则:
  - Interceptor chain 是核心抽象
  - Fail-open: 压缩失败只记录 WARN，透明转发
  - Transport-agnostic: 核心不依赖特定传输实现
  - Config-driven: 运行时行为通过 Config 控制
文档:
  - AGENTS.md 是通用规范 SSOT
  - API 文档由 cargo doc 从代码注释生成
  - 规范行为由 llmanspec/specs 维护（无 mdbook/docs/src）
测试:
  - 单元测试与源码同目录
  - 集成测试在 tests/ 目录
  - 性能测试在 benches/ 目录（criterion）
  - 覆盖率目标: 核心逻辑 ≥90%, Proxy ≥80%

## Artifact 规则

- proposal: 提案保持在 800 字以内
- proposal: 必须包含"非目标"章节
- proposal: 必须说明对现有架构的影响
- proposal: 涉及公共 API 变更时需说明兼容性
- tasks: 每个任务不超过 2 小时
- tasks: 任务描述需明确涉及的文件和模块
- tasks: 测试任务需说明覆盖率目标
- spec: 技术规格需包含代码示例
- spec: 涉及架构变更需更新 AGENTS.md
- spec: API 变更需同步更新 cargo doc 注释

## SDD 流水线

使用 `/llman-sdd-explore` 开始，然后按照 pipeline：`/llman-sdd-propose` → `/llman-sdd-apply` → `/llman-sdd-verify` → `/llman-sdd-archive`。

保留此托管块，便于 `llman sdd init --update` 刷新。
<!-- LLMANSPEC:END -->

# AGENTS.md

Project conventions for lspz (LSP compression proxy).

> **SSOT (Single Source of Truth)**: 本文档是 lspz 项目的通用规范文档。所有开发指南、文档都应引用本文档，避免重复和不一致。

---

## Rust

### Edition & Toolchain

- Edition 2024
- Stable toolchain (pinned in `rust-toolchain.toml`)
- Single crate `lspz` with feature flags (`cli`, `mcp`, `agent-sdk`, `transport-tcp`, `transport-websocket`)

### Code Quality

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
- `just verify` = `scripts/verify-all.sh`（fmt + clippy + test --all-features + doc + SDD validate + prek）
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
2. **llmanspec/specs/** - 行为规格（llman SDD）
3. **README.md** - 用户面向入口
4. **cargo doc** - API 参考（由源码注释生成）

### 文档生成规则 (SSOT)

> **详细规则**: 参见 `llmanspec/specs/ssot-rules/spec.toon`

#### 两层文档体系

| 层 | 工具 | 内容 | 维护方式 |
|----|------|------|----------|
| API 文档 | `cargo doc` | `///` / `//!` 注释 | 编辑源码注释 |
| 规格 | `llman sdd` | 需求与场景 | 编辑 `llmanspec/specs/**` |

#### SSOT 位置

| 内容类型 | SSOT 位置 | 生成命令 | 输出 |
|---------|----------|----------|------|
| API 文档 | `src/**/*.rs` 注释 | `cargo doc --no-deps --all-features` | `target/doc/lspz/` |
| 配置参考 | `src/config.rs` 结构体 | `cargo doc` | `target/doc/lspz/config/` |
| 错误类型 | `src/error.rs` 枚举 | `cargo doc` | `target/doc/lspz/error/` |
| 文档示例 | `///` 代码块 | `cargo test --doc` | CI 验证 |
| 行为规格 | `llmanspec/specs/**` | `llman sdd validate --all` | 规格校验 |

#### 文档命令

```bash
just doc          # 构建 API 文档并在浏览器打开
just doc-check    # 检查 API 文档构建是否成功 (CI 使用)
just doc-test     # 运行文档测试 (验证 /// 示例编译)
```

#### CI 保证

- `cargo doc --no-deps --all-features` — API 文档构建检查
- `cargo test --doc --all-features` — 文档示例编译验证（可单独 `just doc-test`）
- `#![warn(missing_docs)]` — 未文档化公开项产生警告

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
| CI job 总超时 | 15 min | 全流程 (fmt + lint + test + doc-check + doc-test) |
| CI clippy 步骤 | 5 min | 纯静态检查 |
| CI fmt 步骤 | 3 min | 纯静态检查 |
| CI doc-check | 5 min | cargo doc 构建 |
| CI doc-test | 5 min | cargo test --doc |

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

**代码是唯一的真相源 (Single Source of Truth)**。API 文档由 `cargo doc` 从代码注释自动生成；行为规格由 `llmanspec/specs` 维护；用户入口为 README.md。

### 生成规则

| SSOT 位置 | 生成命令 | 输出 |
|-----------|---------|------|
| `///` / `//!` 注释 | `cargo doc --no-deps --all-features` | `target/doc/lspz/` |
| `llmanspec/specs/**` | `llman sdd validate --all` | 规格校验 |

### 架构不变量 (Architecture Invariants)

以下规则在代码实现中必须遵守，任何偏离需在 AGENTS.md 和设计评审中记录：

1. **Interceptor chain 是核心抽象**: 所有 Server→Client 消息转换必须通过 `Interceptor` trait，禁止在 Proxy 核心中直接硬编码消息处理逻辑
2. **Fail-open**: 任何压缩/转换失败 → 只记录 WARN 日志，透明转发原始消息，禁止抛出异常或中断 LSP 通信
3. **Transport-agnostic core**: 核心不依赖任何特定传输实现（stdio/TCP/WS），只依赖 `Transport` trait
4. **Config-driven**: 所有运行时行为通过 `Config` 结构体控制，禁止硬编码开关或行为
5. **Zero-copy preference**: 在 Hot Path（消息收发、拦截器处理）中优先使用引用 `&str` / `&[u8]`、`Cow`、`Arc`，避免不必要的克隆
6. **LSP version locked**: 明确锁定 LSP 3.17 规范，不引入 3.18+ 特性直到项目正式声明支持

### 文档漂移检测

`just qa` 包含以下检查：

1. `cargo fmt --check` + `cargo clippy` + `cargo test` + `cargo doc`（`doc-check`）
2. `prek run --all-files`（hooks）
3. 架构不变量 → 代码审查中人工核对；规格用 `llman sdd validate --all`

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

- 单一 crate，通过 feature flags 控制编译
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

当前发布线为 **v0.11.x**（见 `Cargo.toml` / git tags）。历史里程碑示例：

- v0.1.0: MVP (Library + Proxy)
- v0.2.0: MCP 集成
- v0.3.0: Agent SDK
- v0.10+: Daemon / 文档同步 hardening
- v0.11.x: 当前稳定开发线

### 兼容性承诺

- lspz 公共 API: 主版本不变时向后兼容
- 压缩格式: Agent 端解压库支持所有历史版本
- LSP 兼容性: 永不破坏标准 LSP 协议

---

---

## 检查点规则 (Checkpoint Save)

### 原则

每个阶段或重要模块完成后，必须进行验证 → 提交 → 版本号更新 → 打 tag，以此建立可追溯的检查点链。

### 执行流程

1. **验证**: 运行 `just qa`（fmt-check + lint + test + doc-check + prek）确保无错误
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
| 当前开发线 | v0.11.x | Daemon + MCP + Agent SDK |

### 历史版本管理

- `git tag` 列表展示所有已发布的检查点
- `git diff v0.1.0-alpha.1..v0.1.0-alpha.2` 查看阶段间变更
- 禁止在打 tag 后修改历史（如有紧急修复，递增 patch 版本）

## 参考资源

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [The Rust Style Guide](https://rust-lang.github.io/style-guide/)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [LSP 3.17 Specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
