# lspz

> **lsp** **z**ip - 对 AI Coding Agent 极其友好的 LSP 压缩代理

[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](LICENSE)
[![CI](https://github.com/your-org/lspz/workflows/CI/badge.svg)](https://github.com/your-org/lspz/actions)

## 概述

lspz 是一个**三模态 LSP 压缩代理系统**，通过 Token 敏感的智能压缩，让 AI Coding Agent 用更少上下文理解更多代码问题。

### 核心特性

- **Token 节省**: 诊断消息压缩 ≥40%，降低 API 成本
- **透明集成**: Agent 无需感知，像正常使用 LSP 一样
- **灵活部署**: 支持作为库、独立代理、MCP 服务器三种形态
- **标准兼容**: 永不破坏 LSP 协议标准

### 调研结论

经过对 OpenCode 和 4 个真实 LSP 服务器的分析：

- OpenCode 已经对诊断做了有损压缩（只保留 ERROR，每文件 20 条上限）—— lspz 可以**在协议层以紧凑格式保留 WARNING 信号**，同时保持结构化
- 去重合并是 #1 收益策略：30 个 "unused variable" 错误 → 1 条 + 30 个 range 引用
- 没有现有产品做面向 AI Agent 的 LSP 消息压缩——**lspz 是首创**

---

## 三种产品形态

```
                    ┌─────────────────┐
                    │    lspz-core    │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│Library Mode  │    │ Proxy Mode   │    │  MCP Mode    │
│  #1 优先     │    │  #2 优先     │    │  #3 优先     │
└──────────────┘    └──────────────┘    └──────────────┘
```

| 模式 | 目标用户 | 集成方式 | 典型场景 |
|------|----------|----------|----------|
| **Library** | 自研 Agent CLI | `use lspz_core::Proxy` | 完全控制，零开销 |
| **Proxy** | Claude Code/Continue/Cody | `lspz --backend gopls` | 即插即用，透明代理 |
| **MCP** | 快速实验/多工具协同 | MCP server 配置 | 融入生态，按需查询 |

---

## 快速开始

### 作为库使用（Library Mode）

```toml
[dependencies]
lspz-core = "0.1"
```

```rust
use lspz_core::{Proxy, Config};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::builder()
        .backend_cmd("rust-analyzer")
        .enable_diag_compress(true)
        .build();

    let proxy = Proxy::new(config).await?;
    proxy.initialize().await?;

    let diagnostics = proxy.get_diagnostics(uri).await?;

    Ok(())
}
```

### 作为代理使用（Proxy Mode）

```bash
# 安装
cargo install lspz

# 使用
lspz --backend rust-analyzer --stdio
```

### 作为 MCP 服务器（MCP Mode）

```json
// Claude Desktop 配置
{
  "mcpServers": {
    "lspz": {
      "command": "lspz-mcp",
      "args": ["--backend", "rust-analyzer"]
    }
  }
}
```

---

## 文档

### 新手入门

1. **[ROADMAP.md](ROADMAP.md)** - 项目路线图和愿景
2. **[docs/README.md](docs/README.md)** - 开发者前导（**必读！**）

### 技术文档

- **[docs/specs/001-tri-modal-architecture.md](docs/specs/001-tri-modal-architecture.md)** - 三模态架构规格
- **[docs/specs/002-compression-format.md](docs/specs/002-compression-format.md)** - 压缩格式规范
- **[docs/specs/003-lsp-compatibility.md](docs/specs/003-lsp-compatibility.md)** - LSP 兼容性规范

### 开发指南

- **[docs/plan/01-mvp-phase.md](docs/plan/01-mvp-phase.md)** - MVP 实施计划
- **[docs/guides/coding-conventions.md](docs/guides/coding-conventions.md)** - 编码规范
- **[docs/guides/testing-guide.md](docs/guides/testing-guide.md)** - 测试指南
- **[docs/guides/contributing.md](docs/guides/contributing.md)** - 贡献指南

---

## 项目状态

### 当前版本: v0.2.0-MCP

- [x] 项目规划和文档
- [x] lspz-core 基础实现
- [x] 诊断压缩功能
- [x] LSP 代理模式
- [x] MCP 服务器模式
- [ ] Agent SDK

详细路线图: [ROADMAP.md](ROADMAP.md)

---

## 贡献

欢迎贡献！请阅读 [贡献指南](docs/guides/contributing.md) 了解如何参与。

### 开发前必读

1. [docs/README.md](docs/README.md) - 开发者前导
2. [docs/guides/coding-conventions.md](docs/guides/coding-conventions.md) - 编码规范
3. [docs/guides/testing-guide.md](docs/guides/testing-guide.md) - 测试指南

---

## 许可证

本项目采用双重许可证:

- MIT License ([LICENSE-MIT](LICENSE-MIT) 或 http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) 或 http://www.apache.org/licenses/LICENSE-2.0)

你可以选择其中之一。

---

## 致谢

- [Language Server Protocol](https://microsoft.github.io/language-server-protocol/) - LSP 规范
- [tower-lsp](https://github.com/kinggoesgaming/tower-lsp) - LSP 实现参考
- 所有贡献者

---

## 联系方式

- **GitHub**: https://github.com/your-org/lspz
- **Issues**: https://github.com/your-org/lspz/issues
- **Discussions**: https://github.com/your-org/lspz/discussions
