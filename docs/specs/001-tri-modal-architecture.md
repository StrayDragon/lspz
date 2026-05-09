# lspz 三模态架构规格

**版本**: v0.1.0
**状态**: 草案
**最后更新**: 2026-05-09

## 概述

lspz 采用三模态架构设计，支持作为库、LSP 代理、MCP 服务器三种产品形态。本文档定义这三种模式的设计原则、接口边界和协作方式。

## 架构总览

```
                    ┌─────────────────┐
                    │    lspz-core    │
                    │   (纯逻辑库)     │
                    │                 │
                    │  - Proxy Core   │
                    │  - Interceptors │
                    │  - Codec Layer  │
                    │  - Config       │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│Library Mode │    │ Proxy Mode   │    │  MCP Mode    │
│              │    │              │    │              │
│ lspz-core    │    │ lspz-core    │    │ lspz-core    │
│   + direct   │    │   + CLI      │    │   + MCP      │
│   integration│    │   + stdio    │    │   + tools    │
└──────────────┘    └──────────────┘    └──────────────┘
```

## 模式 1: Library Mode (作为库)

### 设计目标

为自研 Agent CLI 提供完全控制的作为 Rust 库的 LSP 压缩能力，零运行时开销。

### 架构

```rust
// Agent 代码中直接使用
use lspz_core::{Proxy, Config, Transport};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::builder()
        .backend_cmd("rust-analyzer")
        .enable_diag_compress(true)
        .build();

    let proxy = Proxy::new(config).await?;

    // Agent 作为 LSP Client
    proxy.initialize().await?;

    // 处理诊断（已自动压缩）
    let diagnostics = proxy.get_diagnostics(uri).await?;

    // 转换为 Model 友好格式
    let formatted = format_for_model(&diagnostics);

    Ok(())
}
```

### 接口边界

**lspz-core 公共 API**:
- `Proxy`: 核心代理类
- `Config`: 配置构建器
- `Transport`: 传输层 trait
- `Interceptor`: 拦截器 trait

### 特点

| 特性 | 说明 |
|------|------|
| 集成方式 | Cargo 依赖，代码级集成 |
| 传输层 | 可自定义（stdio/TCP/WS） |
| 生命周期 | 由 Agent 控制 |
| 配置方式 | Rust 结构体，类型安全 |
| 适用场景 | 自研 Agent，需要完全控制 |

### 关键约束

1. **API 稳定性**: lspz-core 公共 API 在主版本不变时承诺向后兼容
2. **异步设计**: 所有 I/O 操作基于 tokio 异步
3. **错误处理**: 使用 `Result<T, LspzError>` 统一错误类型

---

## 模式 2: Proxy Mode (LSP 代理)

### 设计目标

为现有 Agent（Claude Code、Continue、Cody）提供即插即用的透明代理。

### 架构

```
┌─────────────┐       ┌─────────────┐       ┌─────────────┐
│             │       │             │       │             │
│   Agent     │◄─────►│    lspz     │◄─────►│ LSP Server  │
│  (LSP Client)│      │   (Proxy)   │       │             │
│             │       │  stdio:0    │       │             │
└─────────────┘       └─────────────┘       └─────────────┘
                            │
                    压缩 Server→Client 消息
                    透明转发 Client→Server 消息
```

### 使用方式

```bash
# 配置 Agent 的 LSP server 为 lspz
lspz --backend rust-analyzer --stdio

# lspz 自动启动真实 LSP server 并代理通信
```

### 接口边界

**stdin/stdout 协议**:
- 输入: 标准 JSON-RPC 2.0 (LSP)
- 输出: 标准 JSON-RPC 2.0 (LSP，但诊断被压缩)

**环境变量配置**:
- `LSPZ_BACKEND_CMD`: 后端 LSP 命令
- `LSPZ_ENABLE_DIAG_COMPRESS`: 启用诊断压缩
- `LSPZ_LOG_LEVEL`: 日志级别

### 特点

| 特性 | 说明 |
|------|------|
| 集成方式 | 独立进程，stdio 通信 |
| 传输层 | 固定使用 stdio |
| 生命周期 | 进程级 |
| 配置方式 | 环境变量 + CLI 参数 |
| 适用场景 | 第三方 Agent，无需修改代码 |

### 关键约束

1. **LSP 兼容性**: 必须完全符合 LSP 3.17 规范
2. **透明性**: Agent 感知不到代理的存在
3. **错误隔离**: 压缩失败不影响 LSP 通信，回退到透明转发

---

## 模式 3: MCP Mode (MCP 服务器)

### 设计目标

为快速实验和多工具协同提供 MCP 协议集成。

### 架构

```
┌─────────────┐       ┌─────────────┐       ┌─────────────┐
│             │       │             │       │             │
│   Agent     │◄─────►│  lspz-mcp   │◄─────►│ LSP Server  │
│ (MCP Client)│       │  (MCP Server)│       │             │
│             │       │             │       │             │
└─────────────┘       └─────────────┘       └─────────────┘
                            │
                    暴露 MCP tools：
                    - get_diagnostics
                    - get_completions
                    - get_symbols
```

### 使用方式

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

### 接口边界

**MCP Tools**:

| Tool | 描述 | 返回值 |
|------|------|--------|
| `get_diagnostics` | 获取文件诊断（压缩格式） | CompactDiagnostics |
| `get_completions` | 获取代码补全 | CompactCompletionItems |
| `get_symbols` | 获取文档符号 | CompactSymbolList |

### 特点

| 特性 | 说明 |
|------|------|
| 集成方式 | MCP server |
| 传输层 | MCP 协议 |
| 生命周期 | Server 进程级 |
| 配置方式 | MCP 配置文件 |
| 适用场景 | 快速实验，多工具协同 |

### 关键约束

1. **MCP 协议**: 符合 MCP 规范
2. **按需查询**: Agent 显式调用 tools
3. **状态管理**: 维护 LSP server 连接池

---

## 核心抽象: lspz-core

### 设计原则

lspz-core 是纯逻辑库，不依赖任何特定传输方式或产品形态。

### 模块结构

```
lspz-core/
├── lib.rs              # 公共 API
├── proxy.rs            # Proxy 核心
├── interceptors/
│   ├── mod.rs
│   └── diagnostics.rs  # 诊断压缩拦截器
├── codec/
│   ├── mod.rs
│   ├── json_rpc.rs     # JSON-RPC 编解码
│   └── compact.rs      # 紧凑格式编解码
├── transport/
│   ├── mod.rs
│   └── stdio.rs        # stdio 传输实现
├── config.rs           # 配置管理
└── error.rs            # 错误类型
```

### 关键 Traits

```rust
// 传输层抽象
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, message: JsonRpcMessage) -> Result<()>;
    async fn receive(&self) -> Result<JsonRpcMessage>;
}

// 拦截器抽象
#[async_trait]
pub trait Interceptor: Send + Sync {
    async fn intercept(
        &self,
        message: &JsonRpcMessage,
        direction: Direction,
    ) -> Result<Option<JsonRpcMessage>>;
}
```

---

## 模式选择指南

### 选择 Library Mode 当：
- ✅ 你控制 Agent 的代码
- ✅ 需要自定义压缩策略
- ✅ 需要零开销集成
- ✅ 使用 Rust 开发 Agent

### 选择 Proxy Mode 当：
- ✅ 使用第三方 Agent（Claude Code/Continue/Cody）
- ✅ 无法修改 Agent 代码
- ✅ 需要快速验证效果
- ✅ 希望透明集成

### 选择 MCP Mode 当：
- ✅ 需要多工具协同
- ✅ 进行快速实验
- ✅ Agent 已支持 MCP
- ✅ 需要按需查询

---

## 兼容性矩阵

| 功能 | Library | Proxy | MCP |
|------|----------|-------|-----|
| 诊断压缩 | ✅ | ✅ | ✅ |
| 自定义传输 | ✅ | ❌ | ❌ |
| 运行时配置 | ✅ | ⚠️ | ⚠️ |
| 热加载 | ✅ | ❌ | ⚠️ |
| 多 server | ✅ | ⚠️ | ✅ |

✅ 完全支持 | ⚠️ 部分支持 | ❌ 不支持

---

## 未来扩展

### v0.2
- [ ] 实现完整的 MCP Mode
- [ ] 添加更多 MCP tools

### v0.3
- [ ] Agent SDK 和宏
- [ ] Skill 生成器

### v0.4
- [ ] TCP/WebSocket 传输层
- [ ] 动态配置热加载
- [ ] Metrics 和 Tracing

---

## 参考文档

- [ROADMAP.md](../../ROADMAP.md) - 项目路线图
- [plan/01-mvp-phase.md](../plan/01-mvp-phase.md) - MVP 实施计划
- [specs/002-compression-format.md](002-compression-format.md) - 压缩格式规范
