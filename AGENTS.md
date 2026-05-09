# AGENTS.md

Project conventions for lspz (LSP compression proxy).

> **SSOT (Single Source of Truth)**: 本文档是 lspz 项目的通用规范文档。所有开发指南、文档都应引用本文档，避免重复和不一致。

---

## Rust

### Edition & Toolchain

- Edition 2021
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

## 参考资源

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [The Rust Style Guide](https://rust-lang.github.io/style-guide/)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [LSP 3.17 Specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
