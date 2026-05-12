# lspz SSOT / 生成物 / 文档治理规则

**版本**: v0.1.0
**状态**: 定稿
**最后更新**: 2026-05-09

## 概述

本文档定义 lspz 项目的 SSOT (Single Source of Truth) 原则和文档生成规则，确保项目内容的一致性和可维护性。

> **原则**: **代码是唯一的真相源**。所有衍生文档从代码注释中自动生成。不确定该改哪里时，先找代码注释；不确定要不要刷新生成物时，先跑 `just qa`。

---

## 1. SSOT 原则

### 核心信条

1. **代码是 SSOT**: 公共 API 的文档注释（`///` / `//!`）是最权威的文档来源
2. **生成物自明**: 任何文件名包含 `.gen.` 的文件都是自动生成的，禁止手工编辑
3. **CI 门禁**: `just qa` 必须包含生成物漂移检测

### 文档编辑规则

| 场景 | SSOT 位置 | 修改方式 | 生成命令 |
|------|----------|---------|----------|
| API 文档 | 代码中的 `///` 和 `//!` 注释 | 编辑源码注释 | `just gen-api-docs` |
| 架构图 | 代码结构（模块、trait 定义） | 编辑源码结构 | `just gen-arch-diagram` |
| 使用示例 | `examples/` 目录下的可运行代码 | 编辑示例代码 | `just gen-examples-docs` |
| 配置参考 | `Config` 结构体定义 | 编辑源码定义 | `just gen-config-docs` |
| 错误类型 | `LspzError` 枚举定义 | 编辑源码定义 | `just gen-error-docs` |
| 项目元数据 | `Cargo.toml` | 编辑 toml 文件 | `just gen-meta-docs` |

---

## 2. SSOT 对照表

### 当前阶段 (v0.1.0-MVP，代码实现中)

| 领域 | SSOT | 生成物 | 生成入口 | 漂移检查 |
|------|------|--------|----------|----------|
| 产品需求 | `docs/plan/00-prd.md` | — | — | — |
| 技术架构 | `docs/specs/001-tri-modal-architecture.md` | — | — | — |
| 压缩格式 | `docs/specs/002-compression-format.md` | `codec/compact.rs` 实现 | — | — |
| LSP 兼容性 | `docs/specs/003-lsp-compatibility.md` | 测试用例 | — | — |
| API 文档 | `src/**/*.rs` 注释 | `docs/api/*.gen.md` | `just gen-api-docs` | `just qa` |
| 模块索引 | `src/lib.rs` 等 crate 入口 | `docs/api/modules.gen.md` | `just gen-api-docs` | `just qa` |
| 使用示例 | `examples/*.rs` | `docs/guides/examples.gen.md` | `just gen-examples-docs` | `just qa` |
| 配置参考 | `src/config.rs` 结构体 | `docs/reference/config.gen.md` | `just gen-config-docs` | `just qa` |
| 错误类型 | `src/error.rs` 枚举 | `docs/reference/error-types.gen.md` | `just gen-error-docs` | `just qa` |
| 项目元数据 | `Cargo.toml` | `README.md` 版本区块 | `just gen-meta-docs` | `just qa` |
| 拦截器 | `Interceptor` trait + 实现 | `docs/specs/interceptors.gen.md` | `just gen-api-docs` | `just qa` |

### MVP 阶段 (v0.1.0 正式版)

| 领域 | SSOT | 生成物 | 生成入口 | 漂移检查 |
|------|------|--------|----------|----------|
| API 文档 | `src/**/*.rs` 中的注释 | `docs/api/*.gen.md` | `just gen-api-docs` | `just qa` |
| 模块索引 | `src/lib.rs` | `docs/api/modules.gen.md` | `just gen-api-docs` | `just qa` |
| 使用示例 | `examples/*.rs` | `docs/guides/examples.gen.md` | `just gen-examples-docs` | `just qa` |
| 配置参考 | `src/config.rs` | `docs/reference/config.gen.md` | `just gen-config-docs` | `just qa` |
| 错误类型 | `src/error.rs` | `docs/reference/error-types.gen.md` | `just gen-error-docs` | `just qa` |
| 项目元数据 | `Cargo.toml` | `README.md` 版本/依赖表格 | `just gen-meta-docs` | `just qa` |

---

## 3. 生成脚本规范

### 接口约定

所有生成脚本 (在 `scripts/` 目录下) 必须支持 `--check` 模式：

```bash
# 写入模式（开发用）
python3 scripts/gen-docs.py

# 检查模式（CI 用）
python3 scripts/gen-docs.py --check
```

`--check` 模式下:
- 如果生成结果与磁盘内容一致 → 退出码 0
- 如果不一致 → 输出 diff，退出码非 0

### 当前脚本

```bash
# 生成所有文档
just gen-docs         # 运行 scripts/gen-docs.py

# 检查文档是否过时
just gen-check        # 运行 scripts/gen-docs.py --check

# 生成特定分类
just gen-api-docs     # 从代码注释生成 API 文档
just gen-config-docs  # 从 Config 结构体生成配置参考
just gen-error-docs   # 从 LspzError 生成错误类型参考
just gen-meta-docs    # 从 Cargo.toml 生成元数据
```

---

## 4. 文件规范

### 命名规则

- **生成文件**: 必须包含 `.gen.` 后缀: `modules.gen.md`, `config.gen.md`
- **注入区块**: 使用 AUTOGEN 注释标记:
  ```markdown
  <!-- BEGIN AUTOGEN:module-list -->
  <!-- END AUTOGEN:module-list -->
  ```

### 代码注释规范

#### 公共 API 文档

```rust
/// # Proxy
///
/// LSP 代理核心，负责转发和拦截 LSP 消息。
///
/// # 架构不变量
///
/// - 所有 Server→Client 消息经过 Interceptor 链
/// - 压缩失败自动降级到透明转发
///
/// # 示例
///
/// ```rust
/// use lspz::{Proxy, Config};
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let config = Config::builder()
///         .backend_cmd("rust-analyzer")
///         .build();
///     let mut proxy = Proxy::new(config, transport).await?;
///     proxy.start().await?;
///     Ok(())
/// }
/// ```
pub struct Proxy;
```

#### 模块级文档

```rust
//! # lspz
//!
//! LSP 压缩代理库。采用三模态架构设计。
//!
//! ## 模块结构
//!
//! - [`proxy`]: 核心代理实现，内部包含 Interceptor 链
//! - [`interceptors`]: 消息拦截器，诊断压缩默认注册
//! - [`codec`]: JSON-RPC 2.0 编解码 + 紧凑格式
//! - [`transport`]: 传输层抽象
```

---

## 5. 漂移检测

### 检查规则

1. **生成物过时**: `.gen.` 文件的修改时间早于 SSOT 文件
2. **手工编辑检测**: `.gen.` 文件的 git diff 显示手工内容变更
3. **注入区块变更**: AUTOGEN 区块内容与生成结果不一致

### 门禁集成

```bash
# justfile
qa: gen-check fmt-check lint test
    @echo "All checks passed!"

gen-check:
    python3 scripts/gen-docs.py --check
```

---

## 6. 参考资源

- [scalim SSOT 规则](.参考项目)
- [Rust 文档注释规范](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html)
- [AGENTS.md](../../AGENTS.md) - 项目开发规范
