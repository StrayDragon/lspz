# SSOT 规则

**版本**: v0.2.0
**最后更新**: 2026-05-13

## 核心原则

**代码是唯一的真相源**。所有 API 文档从代码注释中自动生成。

## 文档分层

| 层 | 工具 | 内容 | 维护方式 |
|----|------|------|----------|
| API 文档 | `cargo doc` | `///` / `//!` 注释 | 编辑源码注释 |
| Book | `mdbook` | 指南、规格、架构 | 手写 Markdown |
| Mermaid 图表 | `mdbook-mermaid` | 架构图、流程图 | `docs/src/diagrams/*.mmd` |

## SSOT 对照表

| 内容 | SSOT 位置 | 生成命令 | 输出 |
|------|----------|----------|------|
| API 文档 | `src/**/*.rs` 注释 | `cargo doc --no-deps --all-features` | `target/doc/lspz/` |
| 配置参考 | `src/config.rs` 结构体 | `cargo doc` | `target/doc/lspz/config/struct.Config.html` |
| 错误类型 | `src/error.rs` 枚举 | `cargo doc` | `target/doc/lspz/error/enum.LspzError.html` |
| 拦截器列表 | `src/interceptors/*.rs` | `cargo doc` | `target/doc/lspz/interceptors/` |
| 文档示例 | `///` 代码块 | `cargo test --doc` | CI 验证 |

## SSOT 保证机制

1. **`#![warn(missing_docs)]`** — CI 中任何未文档化的公开项产生警告
2. **`cargo test --doc`** — CI 中 `///` 代码示例必须编译通过
3. **无生成文件提交** — `cargo doc` 输出在 `target/` 中，不进入 git
4. **Book 源文件手写** — `docs/src/` 下的 Markdown 手动维护

## Mermaid 图表

Mermaid 图表源文件存放在 `docs/src/diagrams/`，通过 mdbook `{{#include}}` 指令嵌入相关章节。

Rust 源码中的 `[MermaidChart:path]` 标记是开发者的交叉引用提示，不会在 rustdoc 中渲染为图表。图表的正式渲染位置是 book。

## 常用命令

```bash
# 构建 API 文档
cargo doc --no-deps --all-features --open

# 运行文档测试
cargo test --doc --all-features

# 构建 book
cd docs && mdbook build

# 本地预览 book（热重载）
cd docs && mdbook serve --open
```
