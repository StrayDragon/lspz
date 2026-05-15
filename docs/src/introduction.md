# lspz

> **lsp** **z**ip — AI-friendly LSP compression proxy

lspz 压缩 LSP 服务器返回的冗余数据，减少 AI Coding Agent 的 token 开销。它坐在 Agent 和 LSP 服务器之间，拦截并压缩诊断、补全、符号等消息。

## 三种运行方式

1. **Library** — 作为 Rust crate 嵌入你的 Agent 项目
2. **CLI Proxy** — 独立进程，透明压缩 LSP 消息
3. **MCP Server** — MCP 工具服务器，供 Claude Desktop 等客户端调用

## 压缩效果

| 消息类型 | Token 节省（紧凑） | Token 节省（TOON） |
|---------|-------------------|-------------------|
| 诊断 | 73.5% | 76.6% |
| 补全 | — | 16.9% |
| Hover | 1.6% | 23.8% |
| 符号 | 33.0% | 72.1% |

## 快速开始

```bash
cargo run -- proxy --backend rust-analyzer
```

详细用法见 [快速开始](./getting-started.md) 和 [架构设计](./architecture.md)。
