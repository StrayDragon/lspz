# lspz

> **lsp** **z**ip — AI-friendly LSP compression proxy

lspz 是一个 LSP 压缩代理，专为 AI Coding Agent 设计。通过压缩 LSP 消息中的冗余数据（诊断、补全、符号等），显著减少 token 消耗。

## 三模态架构

lspz 支持三种运行模式：

1. **Library** — 嵌入式 Rust 库，直接在你的项目中使用
2. **CLI Proxy** — 独立的命令行代理，透明压缩 LSP 消息
3. **MCP Server** — 作为 MCP 工具服务器，供 Claude Desktop 等客户端调用

## 核心特性

- **8 个压缩拦截器**：诊断、补全、Hover、符号、位置等 LSP 消息类型
- **3 种输出格式**：Raw JSON、紧凑 JSON、TOON 表格格式
- **3 种传输层**：stdio、TCP、WebSocket
- **零侵入**：Fail-open 设计，压缩失败自动降级到透明转发

## 快速开始

```bash
# 作为 CLI 代理运行
cargo run -- proxy --backend rust-analyzer

# 查看帮助
cargo run -- --help
```

更多信息请阅读 [Getting Started](./getting-started.md) 和 [Architecture](./architecture.md)。
