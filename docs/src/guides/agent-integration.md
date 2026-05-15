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
│  → rename / code_action / formatting    │
│  → notify_change / notify_close / save  │
│  → send_raw (通用请求)                  │
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
        .workspace_root("/home/user/project")
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

    // 重命名符号
    let edits = agent
        .rename("file:///home/user/project/src/main.rs", 10, 5, "new_name")
        .await?;
    println!("重命名结果: {edits}");

    agent.shutdown().await?;
    Ok(())
}
```

## 多语言支持

使用 `AgentPool` 管理多个 LSP 服务器（内部委托 `AgentHandle`，懒加载）：

```rust,no_run
use lspz::agent_sdk::AgentPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut pool = AgentPool::builder()
        .register("rust", "rust-analyzer")
        .register("go", "gopls")
        .register("python", "basedpyright")
        .workspace_root("/home/user/project")
        .start_all()
        .await?;

    // 查询（按 language id 路由）
    let diags = pool
        .get_diagnostics("file:///home/user/project/src/main.rs", "rust")
        .await?;
    println!("诊断: {diags}");

    pool.shutdown_all().await?;
    Ok(())
}
```

## API 参考

### AgentHandle

单语言 LSP 会话的句柄。

#### 查询方法

| 方法 | 描述 | 返回类型 |
|------|------|---------|
| `get_diagnostics(uri)` | 获取文件的诊断信息 | `String` (JSON/TOON) |
| `get_completions(uri, line, col)` | 获取光标位置的补全 | `String` (JSON) |
| `get_symbols(uri)` | 获取文档符号 | `String` (JSON) |
| `get_hover(uri, line, col)` | 获取悬停信息 | `String` (JSON) |
| `get_references(uri, line, col)` | 查找引用 | `String` (JSON) |
| `get_definition(uri, line, col)` | 转到定义 | `String` (JSON) |
| `get_implementation(uri, line, col)` | 转到实现 | `String` (JSON) |
| `get_type_definition(uri, line, col)` | 转到类型定义 | `String` (JSON) |
| `get_workspace_symbols(query)` | 搜索工作区符号 | `String` (JSON) |
| `get_workspace_diagnostics(uri)` | 获取工作区诊断 | `String` (JSON) |

#### 重构操作

| 方法 | 描述 | 返回类型 |
|------|------|---------|
| `rename(uri, line, col, new_name)` | 重命名符号 | `String` (JSON) |
| `code_action(uri, line, col, diagnostics, only)` | 获取代码操作 | `String` (JSON) |
| `formatting(uri, options)` | 格式化文件 | `String` (JSON) |

#### 文件同步

| 方法 | 描述 | 返回类型 |
|------|------|---------|
| `notify_change(uri, content)` | 通知文件内容变更 (`didChange`) | `()` |
| `notify_close(uri)` | 通知文件关闭 (`didClose`) | `()` |
| `notify_save(uri)` | 通知文件保存 (`didSave`) | `()` |

#### 通用请求

| 方法 | 描述 | 返回类型 |
|------|------|---------|
| `send_raw(method, params)` | 发送任意 LSP 请求（逃生舱口） | `String` (JSON) |

#### 压缩工具

| 方法 | 描述 |
|------|------|
| `compress(raw_json)` | 压缩标准 LSP 诊断为 compact 格式 |
| `inflate(compressed_json)` | 解压 compact 格式回标准 LSP 格式 |

### AgentPool

管理多个语言服务器的池（所有方法委托到对应语言的 `AgentHandle`）。

| 方法 | 描述 |
|------|------|
| `builder()` | 创建 builder |
| `get_diagnostics(uri, language)` | 诊断（委托） |
| `get_completions(uri, language, line, col)` | 补全（委托） |
| `get_symbols(uri, language)` | 符号（委托） |
| `rename(uri, language, line, col, new_name)` | 重命名（委托） |
| `code_action(uri, language, line, col, diags, only)` | 代码操作（委托） |
| `formatting(uri, language, options)` | 格式化（委托） |
| `notify_change(uri, language, content)` | 文件变更（委托） |
| `notify_close(uri, language)` | 文件关闭（委托） |
| `notify_save(uri, language)` | 文件保存（委托） |
| `send_raw(language, method, params)` | 通用请求（委托） |
| `shutdown_all(self)` | 关闭所有服务器 |

## 配置选项

### AgentHandle::builder()

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| `backend` | `&str` | 必需 | LSP 服务器命令 |
| `language` | `&str` | 必需 | 语言标识符 |
| `enable_compression` | `bool` | `false` | 启用消息压缩 |
| `workspace_root` | `&str` | `None` | 工作区根路径（自动补全 `file://`） |

### AgentPool::builder()

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| `register(language, backend)` | — | — | 注册语言后端 |
| `enable_compression` | `bool` | `false` | 启用消息压缩 |
| `workspace_root` | `&str` | `None` | 共享工作区根路径 |

## 文件同步工作流

Agent 编辑文件后，必须通知 LSP server 内容变更：

```rust,no_run
// 1. 初始查询前，文件通过 get_diagnostics 等方法自动打开
let diags = agent.get_diagnostics("file:///project/src/main.rs").await?;

// 2. Agent 编辑文件后，通知 LSP server
agent.notify_change("file:///project/src/main.rs", "fn main() { /* new content */ }").await?;

// 3. 后续查询使用更新后的内容
let diags2 = agent.get_diagnostics("file:///project/src/main.rs").await?;

// 4. 文件不再需要时关闭
agent.notify_close("file:///project/src/main.rs").await?;
```

> `didChange` 使用全量文档同步（发送完整文件内容）。文档版本号自动追踪。

## 错误处理

所有方法都返回 `Result<T, anyhow::Error>`：

```rust,no_run
match agent.get_diagnostics(uri).await {
    Ok(diags) => println!("{diags}"),
    Err(e) => eprintln!("错误: {e}"),
}
```

## 禁用压缩（调试）

```rust,no_run
let agent = AgentHandle::builder()
    .backend("rust-analyzer")
    .language("rust")
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

### Workspace root 重要性

很多 LSP server（rust-analyzer、gopls）需要 workspace root 才能正常工作。不设置时 `rootUri` 为 `null`，可能导致功能降级。

```rust,no_run
let agent = AgentHandle::builder()
    .backend("rust-analyzer")
    .language("rust")
    .workspace_root("/home/user/project")  // 自动转为 file:///home/user/project
    .start()
    .await?;
```

## 相关文档

- [架构文档](../architecture.md) — 理解三模态架构
- [压缩格式](../specs/compression-format.md) — 了解压缩格式
- [测试指南](./testing.md) — 编写测试
