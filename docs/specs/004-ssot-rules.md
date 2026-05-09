# lspz SSOT / 生成物 / 文档治理规则

**版本**: v0.1.0
**状态**: 草案
**最后更新**: 2026-05-09

## 概述

本文档定义 lspz 项目的 SSOT (Single Source of Truth) 原则和文档生成规则，确保项目内容的一致性和可维护性。

> **原则**: 不确定该改哪里时，先找 SSOT；不确定要不要刷新生成物时，先跑 `just qa` 让门禁告诉你。

---

## 1. 文档治理快速规则

### 基本原则

1. **任何文件名包含 `.gen.` 的文件都是全文件生成物**: 不手改；修改 SSOT 后跑 `just gen-*` 入口。
2. **任何 `<!-- BEGIN AUTOGEN:<id> -->` / `<!-- END AUTOGEN:<id> -->` 区块内部是受控注入内容**: 不手改区块内部；修改 SSOT 后跑 `just gen-docs`。

### 文档编辑规则

| 场景 | SSOT 位置 | 修改方式 | 生成命令 |
|------|----------|---------|----------|
| API 文档 | 代码中的 `///` 和 `//!` 注释 | 编辑源码注释 | `just gen-api-docs` |
| 架构图 | 代码结构（模块、trait 定义） | 编辑源码结构 | `just gen-arch-diagram` |
| 使用示例 | `examples/` 目录下的可运行代码 | 编辑示例代码 | `just gen-examples-docs` |
| 配置参考 | `Config` 结构体定义 | 编辑源码定义 | `just gen-config-docs` |
| 项目元数据 | `Cargo.toml` | 编辑 toml 文件 | `just gen-meta-docs` |

---

## 2. SSOT 对照表

### 当前阶段 (v0.1.0-MVP 前)

由于项目处于早期规划阶段，大部分内容暂时以文档为 SSOT。

| 领域 | 当前 SSOT | 未来 SSOT | 状态 |
|------|----------|----------|------|
| 产品需求 | `docs/plan/00-prd.md` | (保持文档为 SSOT) | ✅ 稳定 |
| 技术架构 | `docs/specs/001-tri-modal-architecture.md` | 代码模块结构 | 🔄 迁移中 |
| 压缩格式 | `docs/specs/002-compression-format.md` | Codec 实现 | 🔄 迁移中 |
| LSP 兼容性 | `docs/specs/003-lsp-compatibility.md` | 测试用例 | 🔄 迁移中 |
| API 文档 | (无) | 代码注释 | ⏳ 待实现 |
| 配置说明 | `docs/specs/001-tri-modal-architecture.md` | `Config` 结构体 | ⏳ 待实现 |

### MVP 阶段 (v0.1.0)

| 领域 | SSOT | 生成物 | 生成入口 | 漂移检查 |
|------|------|--------|----------|----------|
| API 文档 | `src/**/*.rs` 中的注释 | `docs/api/*.gen.md` | `just gen-api-docs` | `just qa` |
| 模块索引 | `src/lib.rs` 等 crate 入口 | `docs/api/modules.gen.md` | `just gen-api-docs` | `just qa` |
| 使用示例 | `examples/*.rs` | `docs/guides/examples.gen.md` | `just gen-examples-docs` | `just qa` |
| 配置参考 | `src/config.rs` | `docs/reference/config.gen.md` | `just gen-config-docs` | `just qa` |
| 错误类型 | `src/error.rs` | `docs/reference/error-types.gen.md` | `just gen-error-docs` | `just qa` |
| 项目元数据 | `Cargo.toml` | `README.md` 中的版本/依赖表格 | `just gen-meta-docs` | `just qa` |

---

## 3. 分阶段实施计划

### Phase 0: 当前 (规划阶段)

**目标**: 建立规则基础，文档为临时 SSOT

- [x] 创建 SSOT 规则文档
- [x] 更新 AGENTS.md 添加文档生成规则
- [ ] 创建文档生成脚本框架 (`scripts/gen-docs.py`)
- [ ] 在 justfile 中添加 `gen-*` 命令

### Phase 1: MVP (v0.1.0)

**目标**: 建立代码→文档的生成流程

- [ ] 实现 `lspz-core` 基础模块
- [ ] 为所有公共 API 添加文档注释
- [ ] 创建 API 文档生成脚本
- [ ] 实现漂移检测（检查文档是否过时）
- [ ] 添加到 CI/CD 流程

### Phase 2: 完善 (v0.2.0+)

**目标**: 扩展自动化范围

- [ ] 从测试用例生成使用示例
- [ ] 从代码结构生成架构图
- [ ] 自动生成变更日志
- [ ] 集成到文档站点（MkDocs 或类似）

---

## 4. 文档生成规范

### 文件命名规范

- **生成文件**: 必须包含 `.gen.` 后缀
  - 例如: `modules.gen.md`, `config.gen.md`
- **注入区块**: 使用 AUTOGEN 注释标记
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
/// # 示例
///
/// ```rust
/// use lspz_core::{Proxy, Config};
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let config = Config::builder()
///         .backend_cmd("rust-analyzer")
///         .build();
///
///     let proxy = Proxy::new(config).await?;
///     proxy.initialize().await?;
///
///     Ok(())
/// }
/// ```
///
/// # 错误处理
///
/// 所有公共方法返回 `Result<T, LspzError>`，详见 [`LspzError`]。
///
/// [`LspzError`]: crate::error::LspzError
pub struct Proxy {
    // ...
}
```

#### 模块级文档

```rust
//! # lspz-core
//!
//! LSP 压缩代理核心库。
//!
//! ## 架构概览
//!
//! 本库采用三模态架构设计...
//!
//! ## 模块结构
//!
//! - [`proxy`]: 核心代理实现
//! - [`interceptors`]: 消息拦截器
//! - [`codec`]: 编解码层
//! - [`transport`]: 传输层抽象
```

### 生成脚本规范

所有生成脚本必须支持 `--check` 模式（只检查不写入）：

```bash
# 检查模式（用于 CI）
just gen-api-docs --check

# 写入模式（用于开发）
just gen-api-docs
```

---

## 5. 漂移检测

### 检查规则

1. **生成物过时**: 生成文件的修改时间早于 SSOT
2. **手工编辑检测**: `.gen.` 文件包含手工编辑标记（如 git diff）
3. **注入区块变更**: AUTOGEN 区块内容与生成结果不一致

### 门禁集成

```bash
# justfile
qa:
    just gen-check
    just fmt-check
    just lint
    just test

gen-check:
    cargo run --package gen-docs -- --check
    cargo run --package gen-api-docs -- --check
```

---

## 6. 常见问题

### Q: 为什么现在不立即实现代码→文档生成？

**A**: 当前项目处于规划阶段，代码库几乎是空的。过早建立生成流程会导致：
1. 维护空脚本的开销
2. 频繁调整生成规则
3. 阻碍规划迭代

### Q: 当前应该编辑哪些文件？

**A**:
- ✅ 产品规划: 编辑 `docs/plan/`
- ✅ 技术规格: 编辑 `docs/specs/`
- ✅ 开发指南: 编辑 `docs/guides/`
- ❌ API 文档: 等待有代码后，编辑代码注释

### Q: 什么时候开始实施生成流程？

**A**: 当满足以下条件时：
1. `lspz-core` 有至少 3 个公共模块
2. 有实际的使用示例
3. 文档开始频繁与代码脱节

---

## 7. 参考资源

- [scalim SSOT 规则](../../../../scalim/docs/doc/dev/ssot-map.md) - 参考项目
- [Rust 文档注释规范](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html)
- [AGENTS.md](../../AGENTS.md) - 项目开发规范
