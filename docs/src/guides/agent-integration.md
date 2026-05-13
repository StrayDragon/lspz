# Agent SDK 集成指南

`lspz` 库（feature = "agent-sdk"）提供了用于将 LSP 功能嵌入 AI 编程代理的高级 API。它管理 LSP 服务器进程生命周期、文件同步，并提供类型安全的查询方法。

## 概述

```
┌─────────────────────────────────────────┐
│         你的 AI Agent CLI               │
│    cargo add lspz --no-default-features --features agent-sdk  │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│         lspz (agent-sdk)                 │
│  AgentHandle (单语言)          │
│  AgentPool   (多语言)           │
│  → get_diagnostics / get_completions    │
│  → get_symbols / get_hover              │
│  → get_references / get_definition      │
│  → get_implementation / get_type_def    │
│  → get_workspace_symbols / diagnostics  │
│  → inflate / compress                   │
└────────────────┬────────────────────────┘
                 │ 委托给
┌────────────────▼────────────────────────┐
│    lspz::mcp :: LspSession              │
│  → spawn → initialize → send_request    │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│    lspz::codec::compact                  │
│  → compress / decompress (token 节省)   │
└─────────────────────────────────────────┘
```

## 快速开始（单语言）

添加依赖：

```toml
[dependencies]
lspz = { version = "0.9", default-features = false, features = ["agent-sdk"] }
tokio = { version = "1.35", features = ["full"] }
anyhow = "1.0"
```

基本用法：

```rust,no_run
use lspz::agent_sdk::AgentHandle;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 启动 rust-analyzer 会话
    let mut agent = AgentHandle::builder()
        .backend("rust-analyzer")
        .language("rust")
        .start()
        .await?;

    // 获取文件的诊断
    let diags = agent
        .get_diagnostics("file:///home/user/project/src/main.rs")
        .await?;
    println!("诊断: {diags}");

    // 获取光标位置的补全
    let completions = agent
        .get_completions("file:///home/user/project/src/main.rs", 42, 10)
        .await?;
    println!("补全: {completions}");

    // 获取文档符号
    let symbols = agent
        .get_symbols("file:///home/user/project/src/main.rs")
        .await?;
    println!("符号: {symbols}");

    Ok(())
}
```

## 多语言支持

使用 `AgentPool` 管理多个 LSP 服务器：

```rust,no_run
use lspz::agent_sdk::AgentPool;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut pool = AgentPool::new();

    // 为不同语言启动服务器
    pool.spawn("rust", "rust-analyzer").await?;
    pool.spawn("go", "gopls").await?;
    pool.spawn("python", "pyright").await?;

    // 根据文件扩展名路由查询
    let file = "src/main.rs";
    let language = detect_language(file);  // 你自己实现的
    let agent = pool.get(language).unwrap();

    let symbols = agent.get_symbols(file).await?;
    println!("符号: {symbols}");

    Ok(())
}
```

## API 参考

### AgentHandle

单语言 LSP 会话的句柄。

#### 方法

| 方法 | 描述 | 返回类型 |
|------|------|---------|
| `get_diagnostics(uri)` | 获取文件的诊断信息 | `Vec<CompactDiagnostic>` |
| `get_completions(uri, line, col)` | 获取光标位置的补全 | `Vec<CompactCompletionItem>` |
| `get_symbols(uri)` | 获取文档符号 | `Vec<CompactSymbolInformation>` |
| `get_hover(uri, line, col)` | 获取悬停信息 | `Option<CompactHover>` |
| `get_references(uri, line, col)` | 查找引用 | `Vec<CompactLocation>` |
| `get_definition(uri, line, col)` | 转到定义 | `Vec<CompactLocation>` |
| `get_implementation(uri, line, col)` | 转到实现 | `Vec<CompactLocation>` |
| `get_type_definition(uri, line, col)` | 转到类型定义 | `Vec<CompactLocation>` |
| `get_workspace_symbols(query)` | 搜索工作区符号 | `Vec<CompactSymbolInformation>` |
| `get_workspace_diagnostics()` | 获取工作区诊断 | `Vec<CompactWorkspaceDiagnostic>` |

### AgentPool

管理多个语言服务器的池。

#### 方法

| 方法 | 描述 |
|------|------|
| `new()` | 创建新的空池 |
| `spawn(language, backend)` | 启动指定语言的后端 |
| `get(language)` | 获取指定语言的 agent 句柄 |
| `shutdown()` | 关闭所有服务器 |

## 配置选项

### AgentHandle::builder()

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| `backend` | `&str` | 必需 | LSP 服务器命令 |
| `language` | `&str` | 必需 | 语言标识符 |
| `enable_compression` | `bool` | `true` | 启用消息压缩 |
| `log_level` | `&str` | `"info"` | 日志级别 |

## 错误处理

所有方法都返回 `Result<T, AgentError>`：

```rust,no_run
use lspz::agent_sdk::AgentError;

match agent.get_diagnostics(uri).await {
    Ok(diags) => println!("找到 {} 个诊断", diags.len()),
    Err(AgentError::ServerExited) => eprintln!("服务器崩溃"),
    Err(AgentError::Timeout) => eprintln!("查询超时"),
    Err(e) => eprintln!("错误: {}", e),
}
```

## 高级用法

### 自定义 LSP 服务器参数

```rust,no_run
let agent = AgentHandle::builder()
    .backend("rust-analyzer")
    .args(&["--cli", "--log-file", "/tmp/ra.log"])
    .start()
    .await?;
```

### 禁用压缩（调试）

```rust,no_run
let agent = AgentHandle::builder()
    .backend("rust-analyzer")
    .enable_compression(false)
    .start()
    .await?;
```

## 故障排除

### 服务器无法启动

确保 LSP 服务器在 PATH 中：

```bash
which rust-analyzer  # 应该显示路径
which gopls         # 应该显示路径
```

### 超时错误

增加超时时间：

```rust,no_run
let agent = AgentHandle::builder()
    .backend("rust-analyzer")
    .timeout(Duration::from_secs(30))
    .start()
    .await?;
```

## 相关文档

- [架构文档](../architecture.md) — 理解三模态架构
- [压缩格式](../specs/compression-format.md) — 了解压缩格式
- [测试指南](./testing.md) — 编写测试
