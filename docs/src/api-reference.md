# API Reference

API 文档由 `cargo doc` 自动生成，源码中的 `///` 和 `//!` 注释是唯一的真相源（SSOT）。

## 本地构建

```bash
# 构建 API 文档并在浏览器中打开
cargo doc --no-deps --all-features --open
```

文档输出在 `target/doc/lspz/` 目录下。

## 在线文档

- **docs.rs**：发布到 crates.io 后，API 文档自动部署到 [docs.rs/lspz](https://docs.rs/lspz)
- **GitHub Pages**：开发版文档部署在 GitHub Pages 的 `/api/lspz/` 路径下

## 主要模块

| 模块 | 说明 |
|------|------|
| [`codec`] | 消息编解码层（JSON-RPC、紧凑格式、TOON 格式） |
| [`config`] | 运行时配置 |
| [`error`] | 统一错误类型 |
| [`interceptors`] | 消息拦截器和压缩器 |
| [`metrics`] | 运行时指标 |
| [`proxy`] | LSP 代理核心 |
| [`transport`] | 传输层抽象（stdio、TCP、WebSocket、mock） |
| [`mcp`] | MCP 服务器（需启用 `mcp` feature） |
| [`agent_sdk`] | Agent SDK API（需启用 `agent-sdk` feature） |

## Feature Flags

| Feature | 说明 | 默认 |
|---------|------|------|
| `cli` | CLI 二进制文件 | 是 |
| `mcp` | MCP 服务器 | 否 |
| `agent-sdk` | Agent SDK API（包含 `mcp`） | 否 |
| `transport-tcp` | TCP 传输 | 始终启用 |
| `transport-websocket` | WebSocket 传输 | 否 |
