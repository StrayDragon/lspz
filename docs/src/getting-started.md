# Getting Started

## 安装

```bash
# 从 crates.io 安装（默认启用 cli feature）
cargo install lspz

# 安装并启用 MCP 服务器功能
cargo install lspz --features mcp

# 安装并启用 Agent SDK（隐含 mcp）
cargo install lspz --features agent-sdk

# 从源码构建（需要 Rust 2024 edition）
cargo build --release
```

## 作为 CLI Proxy 使用

```bash
# 基本用法：透明代理到 rust-analyzer
lspz proxy --backend rust-analyzer

# 使用紧凑 JSON 输出格式
lspz proxy --backend rust-analyzer --format compact

# 使用 TOON 表格输出格式（推荐 AI Agent 使用）
lspz proxy --backend rust-analyzer --format toon

# 通过 TCP 监听
lspz proxy --backend rust-analyzer --listen 127.0.0.1:9000
```

## 作为 Library 使用

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
lspz = "0.11"
```

基本使用示例：

```rust
use lspz::{Proxy, Config, StdioTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::builder()
        .backend_cmd("rust-analyzer")
        .build();

    let transport = StdioTransport::new(config.backend_cmd.clone());
    let mut proxy = Proxy::new(config, transport).await?;

    proxy.start().await?;
    Ok(())
}
```

## 作为 MCP Server 使用

启用 `mcp` feature：

```toml
[dependencies]
lspz = { version = "0.11", features = ["mcp"] }
```

## 运行示例

```bash
# 压缩演示（快速验证）
cargo run --example compress-demo

# 基准测试报告
cargo run --example bench-report
```

## 下一步

- [Architecture](./architecture.md) — 理解三模态架构设计
- [Agent Integration](./guides/agent-integration.md) — Agent SDK 使用方法
- [Compression Format](./specs/compression-format.md) — 紧凑格式规范
