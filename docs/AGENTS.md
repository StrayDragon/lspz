# lspz Documentation

> 文档分为两层：**API 文档**（自动生成）和 **Book**（手动维护）。

## API 文档 (cargo doc)

自动从源码 `///` / `//!` 注释生成。

```bash
# 构建并在浏览器中打开
cargo doc --no-deps --all-features --open
```

输出位置：`target/doc/lspz/index.html`

## Book (mdbook)

手写文档：指南、规格、架构设计。

```bash
# 构建
cd docs && mdbook build

# 本地预览（热重载）
cd docs && mdbook serve --open
```

输出位置：`docs/book/index.html`

## 文档结构

```
docs/
├── book.toml              # mdbook 配置
├── mermaid.min.js         # Mermaid 渲染库
├── mermaid-init.js        # Mermaid 初始化
├── src/                   # mdbook 源文件
│   ├── SUMMARY.md         # 目录导航
│   ├── introduction.md    # 项目介绍
│   ├── getting-started.md # 快速开始
│   ├── architecture.md    # 架构设计
│   ├── guides/            # 开发指南
│   ├── specs/             # 技术规格
│   ├── diagrams/          # Mermaid 图表源文件
│   └── ...
└── LSP-Specification.html # LSP 3.17 规范（静态参考）
```

## 常用命令

| 命令 | 说明 |
|------|------|
| `just doc` | 构建 API 文档并打开 |
| `just doc-check` | 检查 API 文档构建（CI） |
| `just doc-test` | 运行文档测试 |
| `just book` | 构建 book |
| `just book-serve` | 本地预览 book |
