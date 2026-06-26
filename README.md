<div align="center">
  <img src="docs/assets/logo.svg" alt="lspz" width="160" height="160"/>

  # lspz

  [![](https://img.shields.io/crates/v/lspz?style=flat-square&logo=rust&label=crates.io)](https://crates.io/crates/lspz)
  [![](https://img.shields.io/docsrs/lspz?style=flat-square&logo=docsdotrs&label=docs.rs)](https://docs.rs/lspz)
  [![](https://img.shields.io/crates/l/lspz?style=flat-square&color=blue)](https://github.com/straydragon/lspz/blob/main/LICENSE)
  [![](https://img.shields.io/badge/edition-2024-orange?style=flat-square)](https://blog.rust-lang.org/2025/02/20/Rust-2024-Edition.html)
  [![](https://img.shields.io/github/actions/workflow/status/straydragon/lspz/ci.yml?style=flat-square&logo=github&label=CI)](https://github.com/straydragon/lspz/actions)

  **lsp** **z**ip — 压缩 LSP 消息，给 AI 智能体省 token

  [快速开始](docs/src/getting-started.md) · [架构设计](docs/src/architecture.md) · [API 文档](https://docs.rs/lspz) · [文档书](docs/src/SUMMARY.md)
</div>

---

lspz 跑在 AI 编码智能体和 LSP 服务器之间。它拦截服务器响应（诊断、补全、符号、悬停等），重写成更紧凑的格式，少用不少 token。按 token 计费或者上下文窗口有限的时候尤其有用。

## 工作原理

```
Agent (LSP 客户端) ←→ lspz ←→ LSP 服务器 (rust-analyzer, gopls, ...)
```

客户端发给服务器的消息原样透传。服务器返回的响应经过一组拦截器：去掉冗余字段、去重、编码成紧凑格式。任何拦截器出错就转发原始消息——不会搞坏你的 LSP 会话。

## 三种用法

**库** — 嵌入你自己的 Rust 智能体：

```toml
[dependencies]
lspz = { version = "0.11", default-features = false }
```

**CLI 代理** — 直接替换你的 LSP 服务器命令：

```bash
lspz proxy --backend rust-analyzer
```

**MCP 服务器** — 把 LSP 能力暴露为 MCP 工具，给 Claude Desktop 等客户端用：

```bash
lspz mcp
```

## 压缩效果

用 tiktoken 在真实 LSP 服务器输出上测的。TOON（Token-Oriented Object Notation）是默认输出格式。

| 拦截器 | LSP 方法 | 紧凑格式节省 | TOON 节省 |
|---|---|---|---|
| DiagnosticsCompressor | `textDocument/publishDiagnostics` | 73.5% | 76.6% |
| HoverCompressor | `textDocument/hover` | 1.6% | 23.8% |
| DocumentSymbolCompressor | `textDocument/documentSymbol` | 33.0% | 72.1% |
| CompletionCompressor | `textDocument/completion` | — | 16.9% |
| LocationCompressor | references/definition/... | 60–85% | — |
| WorkspaceDiagnosticCompressor | `workspace/diagnostic` | 83–90% | — |
| WorkspaceSymbolCompressor | `workspace/symbol` | 70–76% | — |
| CappingInterceptor | 任意大响应 | 80–95% | — |

完整数据见 [benchmarks.md](docs/src/benchmarks.md)。

## 输出格式

| 格式 | 说明 | 适用场景 |
|---|---|---|
| `toon`（默认） | 自描述行协议 + 表格 | LLM 直接消费 |
| `compact` | 缩短 JSON 字段名 | 需要结构化数据时 |
| `passthrough` | 原始 LSP JSON 不动 | 调试 |

## Feature flags

| Flag | 启用内容 | 默认 |
|---|---|---|
| `cli` | `lspz` 二进制（clap、tracing-subscriber） | 开 |
| `mcp` | MCP 服务器（rmcp） | 关 |
| `agent-sdk` | AgentHandle + AgentPool（隐含 `mcp`） | 关 |
| `transport-tcp` | TcpTransport | 关（代码始终包含） |
| `transport-websocket` | WsTransport | 关 |

## Agent SDK

把 LSP 能力嵌入你自己的智能体。支持 10 种查询方法、文件同步、重构操作：

```rust
use lspz::agent_sdk::AgentHandle;

let mut agent = AgentHandle::builder()
    .backend("rust-analyzer")
    .language("rust")
    .workspace_root("/home/user/project")
    .start()
    .await?;

let diags = agent.get_diagnostics("file:///home/user/project/src/main.rs").await?;
let completions = agent.get_completions("file:///home/user/project/src/main.rs", 42, 10).await?;
let edits = agent.rename("file:///home/user/project/src/main.rs", 10, 5, "new_name").await?;

agent.shutdown().await?;
```

完整 API 见 [Agent 集成指南](docs/src/guides/agent-integration.md)。

## 安装

```bash
# 从 crates.io（默认启用 cli feature）
cargo install lspz

# 启用 MCP 服务器
cargo install lspz --features mcp

# 启用 Agent SDK（隐含 mcp）
cargo install lspz --features agent-sdk

# 从源码
git clone https://github.com/straydragon/lspz && cd lspz
cargo install --path .
```

## 快速验证

```bash
cargo run --example compress-demo   # 展示各拦截器的 token 节省
cargo bench                         # Criterion 吞吐量基准
just qa                             # fmt + clippy + test + doc-check
```

## 文档

- [文档书](docs/src/SUMMARY.md) — 架构、指南、规格
- [API 参考](https://docs.rs/lspz) — 从 `///` 注释自动生成
- [CHANGELOG](CHANGELOG.md) — 版本历史
- [ROADMAP](ROADMAP.md) — 已完成和计划中的功能

## 许可证

MIT
