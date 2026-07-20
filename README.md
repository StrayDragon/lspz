<div align="center">
  <img src="docs/assets/logo.svg" alt="lspz" width="160" height="160"/>

  # lspz

  [![](https://img.shields.io/crates/v/lspz?style=flat-square&logo=rust&label=crates.io)](https://crates.io/crates/lspz)
  [![](https://img.shields.io/crates/dr/lspz?style=flat-square&logo=rust)](https://crates.io/crates/lspz)
  [![](https://img.shields.io/docsrs/lspz?style=flat-square&logo=docsdotrs&label=docs.rs)](https://docs.rs/lspz)
  [![](https://img.shields.io/github/stars/straydragon/lspz?style=flat-square&logo=github)](https://github.com/straydragon/lspz/stargazers)
  [![](https://img.shields.io/github/actions/workflow/status/straydragon/lspz/ci.yml?style=flat-square&logo=github&label=CI)](https://github.com/straydragon/lspz/actions)
  [![](https://img.shields.io/crates/l/lspz?style=flat-square&color=blue)](https://github.com/straydragon/lspz/blob/main/LICENSE)

  **lsp** **z**ip — 压缩 LSP 消息，给 AI 智能体省 token

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
lspz = { version = "0.11", default-features = false, features = ["agent-sdk"] }
```

**CLI 代理** — 直接替换你的 LSP 服务器命令：

```bash
lspz proxy --backend rust-analyzer
```

**MCP 服务器** — 把 LSP 能力暴露为 MCP 工具，给 Cursor / Claude Desktop 等客户端用（需 `--features mcp`）：

```bash
cargo install lspz --features mcp
lspz mcp
```

对 Cursor 等编码 Agent：`get_diagnostics` / `get_symbols` 的 `uri` 可省略——会通过 MCP Roots（否则进程 cwd）解析工作区，扫描少量源文件并返回 dense TOON 总览。`get_completions` 仍需要 `uri` + 光标位置。
## 压缩效果

用 tiktoken 在真实 LSP 服务器输出上测的。TOON（Token-Oriented Object Notation）是默认输出格式。

| 拦截器 | LSP 方法 | 紧凑 vs 原始 | TOON vs 原始 |
|---|---|---|---|
| DiagnosticsCompressor | `textDocument/publishDiagnostics` | 73.5% | 76.6% |
| DocumentSymbolCompressor | `textDocument/documentSymbol` | 33.0% | 72.1% |
| HoverCompressor | `textDocument/hover` | 1.6% | 23.8% |
| WorkspaceDiagnosticCompressor | `workspace/diagnostic` | 26.2% | 29.3% |
| CompletionCompressor | `textDocument/completion` | -0.7% | 16.9% |
| WorkspaceSymbolCompressor | `workspace/symbol` | -5.3% | 36.7% |
| LocationCompressor | `textDocument/references` 等 | -19.1% | 13.9% |

> 紧凑格式对小输入有 JSON 字段名开销，诊断和符号等大响应收益显著。
> TOON 是默认格式，在所有场景下都能稳定节省 token。

完整数据用 `cargo run --example bench-report` 生成。

## 输出格式

| 格式 | 说明 | 适用场景 |
|---|---|---|
| `toon`（默认） | 自描述行协议 + 表格 | LLM 直接消费 |
| `json` | 缩短字段名的 compact JSON | 需要结构化数据时 |
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

完整 API 见 [Agent 集成指南](https://docs.rs/lspz/latest/lspz/agent_sdk/)。

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
just verify                         # full harness (qa + doc-test + SDD + prek)
```

## 文档

- [API 参考](https://docs.rs/lspz) — 从 `///` 注释自动生成
- [LSP 规范](docs/LSP-Specification.html) — LSP 3.17 规范参考
- [AGENTS.md](AGENTS.md) — 项目约定与架构不变量
- [llmanspec/](llmanspec/) — SDD 规格（`llman sdd list --specs`）

## 许可证

MIT
