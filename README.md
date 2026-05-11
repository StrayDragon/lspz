# lspz

> **lsp** **z**ip — 对 AI Coding Agent 极其友好的 LSP 压缩代理

[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](LICENSE)
[![CI](https://github.com/your-org/lspz/workflows/CI/badge.svg)](https://github.com/your-org/lspz/actions)

## 概述

lspz 是一个**三模态 LSP 压缩代理系统**，通过 Token 敏感的智能压缩，让 AI Coding Agent
用更少上下文理解更多代码问题。

### 核心特性

- **Token 节省**: 4 种 LSP 消息类型压缩，平均节省 40–80%
- **透明集成**: Agent 无需感知，像正常使用 LSP 一样
- **灵活部署**: 支持作为库、独立代理、MCP 服务器三种形态
- **标准兼容**: 永不破坏 LSP 协议标准（fail-open 保证）

### 已实现的 4 个压缩器

| 压缩器 | 消息类型 | 策略 | Token 节省 |
|--------|----------|------|------------|
| **Diagnostics** | `publishDiagnostics` | 去重合并 + 字段裁剪 + range delta 编码 | 40–80% |
| **Completion** | `textDocument/completion` | 截断 + 字段裁剪 + kind 编码 + doc 去重 | 30–60% |
| **Hover** | `textDocument/hover` | Markdown 压缩 + 字段裁剪 + MarkupKind 缩减 | 20–40% |
| **DocumentSymbol** | `textDocument/documentSymbol` | 递归树压缩 + SymbolKind 编码 + 字段裁剪 | 30–50% |

### 一键验证

```bash
bash scripts/compress-demo.sh
```

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

### Proxy 模式（推荐体验方式）

```bash
# 安装
cargo install lspz

# 直接使用（以 rust-analyzer 为例）
lspz --backend rust-analyzer

# 自定义配置
lspz --backend gopls -d -c -H -S           # 全部启用（默认）
lspz --backend gopls --compress-diag false   # 仅禁用诊断压缩
lspz --backend gopls -l debug                # 调试日志
```

### 作为库使用（Library Mode）

```toml
[dependencies]
lspz-core = "0.1"
```

```rust
use lspz_core::{Config, Proxy, StdioTransport};
use lspz_core::interceptors::{Interceptor, InterceptorChain, Direction};
use lspz_core::DiagnosticsCompressor;

let config = Config::builder()
    .backend_cmd("rust-analyzer")
    .enable_diag_compress(true)
    .build()?;

let transport = StdioTransport::spawn("rust-analyzer")?;
let chain = InterceptorChain::new(vec![
    Box::new(DiagnosticsCompressor::default()),
]);

let mut proxy = Proxy::new(config, Box::new(transport), chain);
proxy.start().await?;
```

### 作为 MCP 服务器（MCP Mode）

```json
{
  "mcpServers": {
    "lspz": {
      "command": "lspz",
      "args": ["mcp"]
    }
  }
}
```

---

## 验证压缩效果

```bash
# 一键验证（无需 LSP 服务器）
bash scripts/compress-demo.sh

# 输出示例:
#   📦 DiagnosticsCompressor           712B → 340B   (52.2%  μs=42)
#   📦 CompletionCompressor            562B → 348B   (38.1%  μs=28)
#   📦 HoverCompressor                 254B → 184B   (27.6%  μs=15)
#   📦 DocumentSymbolCompressor        416B → 200B   (51.9%  μs=12)
#   ────────────────────────────────────────────────────
#   TOTAL:     1944 bytes → 1072 bytes  (44.9% saved)
```

---

## 项目结构

```
crates/
├── lspz-core/         # 核心库 — Proxy, Transport, Interceptor 链
├── lspz/              # CLI — Proxy + MCP server 入口
├── lspz-mcp/          # MCP server crate
└── lspz-agent-sdk/    # Agent SDK — AgentHandle + AgentPool

scripts/
├── compress-demo.sh   # 一键压缩验证脚本
└── gen-docs.py        # 文档生成器（SSOT）
```

---

## 质量

```
cargo clippy      ── 零警告
cargo fmt --check ── 通过
cargo test        ── 97+ 测试通过（3 suites）
just gen-check    ── 17/17 文档同步
```

---

## 项目状态

### 当前版本: v0.5.0

- ✅ 所有 4 个 LSP 消息类型压缩器已实现
- ✅ Proxy / Library / MCP 三种模式
- ✅ Agent SDK（AgentHandle + AgentPool）
- ✅ 97+ 测试，clippy 零警告，文档 SSOT 同步
- ✅ fail-open：任何压缩失败不影响消息转发

### 路线图

参见 [ROADMAP.md](ROADMAP.md) 了解详细版本计划和未来方向。

---

## 许可证

本项目采用双重许可证:

- MIT License ([LICENSE-MIT](LICENSE-MIT) 或 http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) 或 http://www.apache.org/licenses/LICENSE-2.0)
