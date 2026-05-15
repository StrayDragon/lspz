# lspz 三模态架构规格

**版本**: v0.9.2
**状态**: 定稿
**最后更新**: 2026-05-15

## 概述

lspz 采用三模态架构设计，支持作为库、LSP 代理、MCP 服务器三种产品形态。本文档定义这三种模式的设计原则、接口边界和协作方式。

---

## 架构总览

```mermaid
graph TB
    subgraph Core["lspz crate (feature flags)"]
        PROXY["Proxy Core<br/>生命周期管理<br/>消息路由"]
        INTERCEPTOR["Interceptor Chain<br/>转换/压缩/过滤"]
        CODEC["Codec Layer<br/>JSON-RPC 编解码<br/>紧凑格式"]
        TRANSPORT["Transport Trait<br/>抽象 I/O 通道"]
        MCPMOD["MCP Server<br/>(feature = mcp)"]
        AGENT["Agent SDK<br/>(feature = agent-sdk)"]
        CONFIG["Config<br/>Builder + Env"]
        ERROR["Error Types<br/>thiserror"]
    end

    subgraph Delivery["交付形态"]
        LIB["Library Mode<br/>use lspz::Proxy"]
        CLIBIN["Proxy Mode<br/>lspz --backend ra"]
        MCP["MCP Mode<br/>lspz mcp"]
    end

    PROXY --> INTERCEPTOR
    PROXY --> CODEC
    PROXY --> TRANSPORT
    PROXY --> CONFIG
    PROXY --> ERROR
    INTERCEPTOR --> CODEC
    CODEC --> ERROR
    MCPMOD --> PROXY
    AGENT --> MCPMOD
    LIB --> PROXY

    classDef core fill:#4A90E2,stroke:#2E5C8A,stroke-width:2px,color:#fff
    classDef delivery fill:#7ED321,stroke:#5BA01A,stroke-width:2px,color:#fff
    class Core core
    class LIB,CLIBIN,MCP delivery
```

---

## 完整消息流

```mermaid
sequenceDiagram
    participant Agent as AI Agent (LSP Client)
    participant Proxy as lspz Proxy
    participant Server as LSP Server

    Note over Agent,Server: Phase 1: Handshake
    Agent->>Proxy: initialize (params + clientCapabilities)
    Proxy->>Server: initialize (forward, unmodified)
    Server-->>Proxy: capabilities + serverInfo
    Proxy-->>Agent: capabilities (transparent forward)
    Agent->>Proxy: initialized notification
    Proxy->>Server: initialized (forward)

    Note over Agent,Server: Phase 2: Document Lifecycle
    Agent->>Proxy: textDocument/didOpen
    Proxy->>Server: didOpen (forward)
    Note over Server: Analyzes file
    Server-->>Proxy: textDocument/publishDiagnostics (raw)
    Note over Proxy: Interceptor Chain executes
    Note over Proxy: ① Dedup (msg+severity)
    Note over Proxy: ② Prune fields
    Note over Proxy: ③ Encode enums
    Note over Proxy: ④ Delta encode ranges
    Proxy-->>Agent: publishDiagnostics (compact)

    Note over Agent,Server: Phase 3: Transparent Passthrough
    Agent->>Proxy: textDocument/hover
    Proxy->>Server: hover (forward)
    Server-->>Proxy: hover result
    Proxy-->>Agent: hover result (unmodified)
```

---

## Proxy 核心状态机

```mermaid
stateDiagram-v2
    [*] --> Created: Proxy::new(config)
    Created --> Initializing: initialize()
    Initializing --> Ready: initialized received
    Initializing --> ShuttingDown: shutdown received
    Ready --> Working: message loop
    Working --> Ready: continue
    Working --> ShuttingDown: shutdown received
    ShuttingDown --> Exited: exit notification
    Exited --> [*]

    state Ready {
        [*] --> Idle
        Idle --> Forwarding: Client→Server msg
        Forwarding --> Idle: msg forwarded
        Idle --> Intercepting: Server→Client msg
        Intercepting --> Compressing: matched interceptor
        Intercepting --> Forwarding: no match (passthrough)
        Compressing --> Idle: compressed msg sent
        Compressing --> Forwarding: compression failed (fallback)
    }
```

---

## 拦截器链架构

```mermaid
flowchart TB
    subgraph Incoming["Server to Client Messages"]
        MSG["LSP Message (JSON-RPC 2.0)"]
    end

    subgraph Chain["Interceptor Chain (8 interceptors, config-gated)"]
        direction TB
        C["CappingInterceptor<br/>截断大返回"]
        D["DiagnosticsCompressor<br/>去重 + delta range"]
        CO["CompletionCompressor<br/>Kind 编码 + doc 去重"]
        H["HoverCompressor<br/>Markdown 折叠"]
        S["DocumentSymbolCompressor<br/>SymbolKind 编码"]
        L["LocationCompressor<br/>URI 去重"]
        WS["WorkspaceSymbolCompressor<br/>workspace/symbol"]
        WD["WorkspaceDiagnosticCompressor<br/>workspace/diagnostic"]
    end

    subgraph ErrorPath["Error Handling (Fail-Open)"]
        ERR["Compression Failed"]
        FALLBACK["Transparent Forward"]
        LOG["Log WARN + Continue"]
        ERR --> FALLBACK
        ERR --> LOG
    end

    subgraph Outgoing["To AI Agent"]
        COMPRESSED["Compressed / TOON"]
        RAW["Original Message"]
    end

    MSG --> C --> D --> CO --> H --> S --> L --> WS --> WD
    WD -->|Success| COMPRESSED
    WD -->|Failure| ERR
    FALLBACK --> RAW

    classDef incoming fill:#7ED321,stroke:#5BA01A,stroke-width:2px,color:#fff
    classDef chain fill:#4A90E2,stroke:#2E5C8A,stroke-width:2px,color:#fff
    classDef error fill:#F5A623,stroke:#D4880F,stroke-width:2px,color:#fff
    classDef outgoing fill:#9013FE,stroke:#6A0DAD,stroke-width:2px,color:#fff

    class MSG incoming
    class C,D,CO,H,S,L,WS,WD chain
    class ERR,FALLBACK,LOG error
    class COMPRESSED,RAW outgoing
```

---

## Proxy 消息循环

```mermaid
flowchart TB
    subgraph 初始化["初始化阶段"]
        I1["Proxy::start()"]
        I2["发送 initialize 请求"]
        I3["等待 initialize 响应"]
        I4["发送 initialized 通知"]
        I5["保存 ServerCapabilities"]
    end

    subgraph 主循环["消息循环"]
        L1["等待下一条消息 (tokio::select!)"]
        L2["消息来源?"]
        L3["Client to Server: 透明转发给 LSP 后端"]
        L4["Server to Client: 进入拦截器链"]
        L5["拦截器链处理"]
        L6["发送给 Client"]
    end

    subgraph 关闭["关闭流程"]
        S1["shutdown() 被调用"]
        S2["发送 shutdown 请求"]
        S3["发送 exit 通知"]
        S4["杀死 server 子进程"]
    end

    I1 --> I2 --> I3 --> I4 --> I5
    I5 --> L1
    L1 --> L2
    L2 -->|"来自 Client (stdin)"| L3 --> L1
    L2 -->|"来自 Server (子进程 stdout)"| L4 --> L5 --> L6 --> L1
    L1 -->|"退出信号 / 错误"| S1 --> S2 --> S3 --> S4

    classDef init fill:#4A90E2,stroke:#2E5C8A,stroke-width:2px,color:#fff
    classDef loop fill:#7ED321,stroke:#5BA01A,stroke-width:2px,color:#fff
    classDef end fill:#F5A623,stroke:#D4880F,stroke-width:2px,color:#fff

    class I1,I2,I3,I4,I5 init
    class L1,L2,L3,L4,L5,L6 loop
    class S1,S2,S3,S4 end
```

**消息循环关键细节**:

- 使用 `tokio::select!` 同时等待 Client stdin 和 Server stdout 两条消息源
- Client→Server 消息**不做任何处理**，直接序列化写入子进程 stdin
- Server→Client 消息**通过拦截器链后再发送**给 Client
- 拦截器链中任何一步失败 → `tracing::warn!` + 透明转发原始消息
- 信号处理: SIGTERM/SIGINT → 触发 `shutdown()` → 优雅关闭

---

## 模式 1: Library Mode (作为库)

### 设计目标

为自研 Agent CLI 提供完全控制的 Rust 库 LSP 压缩能力，零运行时开销。

### 核心 Traits

```rust
/// 传输层抽象，定义 I/O 通道的基本操作。
///
/// # 实现者
///
/// - `StdioTransport` (MVP)
/// - `TcpTransport`
/// - `WsTransport` (feature-gated)
#[async_trait]
pub trait Transport: Send + Sync {
    /// 接收一条原始 LSP 消息（按 Content-Length 分割）。
    async fn receive(&mut self) -> Result<Vec<u8>, LspzError>;
    /// 发送一条原始 LSP 消息。
    async fn send(&mut self, data: &[u8]) -> Result<(), LspzError>;
}

/// 拦截器 trait，核心扩展点。
///
/// 每个拦截器可以检查并选择性转换消息。
/// 拦截器链在 Proxy 内部按注册顺序执行。
#[async_trait]
pub trait Interceptor: Send + Sync {
    /// 拦截器唯一名称（用于排序和配置）。
    fn name(&self) -> &str;
    /// 判断是否处理该消息。
    fn applies_to(&self, method: &str, direction: Direction) -> bool;
    /// 转换消息 params。Ok(None) = 丢弃，Ok(Some(params)) = 替换。
    /// 默认实现：不做任何修改。
    async fn intercept(
        &self,
        method: &str,
        params: serde_json::Value,
        direction: Direction,
    ) -> Result<Option<serde_json::Value>, LspzError> {
        Ok(Some(params))
    }
}

/// 消息方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    ClientToServer,
    ServerToClient,
}

/// 解析后的 LSP 消息。
#[derive(Debug)]
pub enum LspMessage {
    Request { id: i64, method: String, params: serde_json::Value },
    Response { id: i64, result: Option<serde_json::Value>, error: Option<JsonRpcError> },
    Notification { method: String, params: serde_json::Value },
}
```

### 使用示例

```rust
use lspz::{Proxy, Config, StdioTransport};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::builder()
        .backend_cmd("rust-analyzer")
        .enable_diag_compress(true)
        .build()?;

    let transport = StdioTransport::new("rust-analyzer")?;
    let mut proxy = Proxy::new(config, Box::new(transport));

    proxy.start().await?;          // 执行握手
    proxy.open_file(uri).await?;   // 打开文件
    let diags = proxy.diagnostics(uri).await?; // 获取（已压缩的）诊断

    // diags 已经是 CompactDiagnostics 格式
    Ok(())
}
```

### 关键约束

1. **API 稳定性**: 公共 API 在主版本不变时承诺向后兼容
2. **异步设计**: 所有 I/O 操作基于 tokio 异步
3. **错误处理**: 使用 `Result<T, LspzError>` 统一错误类型

### 特点

| 特性 | 说明 |
|------|------|
| 集成方式 | Cargo 依赖，代码级集成 |
| 传输层 | 可自定义（stdio/TCP/WS） |
| 生命周期 | 由 Agent 控制 |
| 配置方式 | Rust 结构体，类型安全 |
| 适用场景 | 自研 Agent，需要完全控制 |

---

## 模式 2: Proxy Mode (LSP 代理)

### 设计目标

为现有 Agent（Claude Code、Continue、Cody、OpenCode）提供即插即用的透明代理。

### 架构

```mermaid
flowchart LR
    subgraph Client["Agent (LSP Client)"]
        C1["stdio: client side"]
    end
    subgraph Proxy["lspz Proxy"]
        P1["stdio client<br/>read/write"]
        P2["Proxy Core"]
        P3["stdio server<br/>read/write"]
    end
    subgraph Server["LSP Server"]
        S1["stdio: server side"]
    end

    C1 <-->|"stdin/stdout"| P1
    P1 --- P2
    P2 --- P3
    P3 <-->|"stdin/stdout"| S1

    classDef client fill:#7ED321,stroke:#5BA01A,stroke-width:2px,color:#fff
    classDef proxy fill:#4A90E2,stroke:#2E5C8A,stroke-width:2px,color:#fff
    classDef server fill:#F5A623,stroke:#D4880F,stroke-width:2px,color:#fff
    class C1 client
    class P1,P2,P3 proxy
    class S1 server
```

### 使用方式

```bash
# 配置 Agent 的 LSP server 为 lspz
cargo run -- --backend rust-analyzer

# lspz 自动启动真实 LSP server 并代理通信
# 所有 Server→Client 的诊断消息被自动压缩
```

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

```mermaid
flowchart LR
    subgraph Client["Agent (MCP Client)"]
        C1["MCP Protocol"]
    end
    subgraph Mcp["lspz (feature = mcp)"]
        M1["MCP Tools:<br/>- get_diagnostics<br/>- get_completions<br/>- get_symbols"]
        M2["lspz core"]
        M3["LSP Server Pool"]
    end
    subgraph Servers["LSP Servers"]
        S1["rust-analyzer"]
        S2["gopls"]
        S3["basedpyright"]
    end

    C1 <--> M1
    M1 --- M2
    M2 --- M3
    M3 --- S1
    M3 --- S2
    M3 --- S3

    classDef client fill:#7ED321,stroke:#5BA01A,stroke-width:2px,color:#fff
    classDef mcp fill:#9013FE,stroke:#6A0DAD,stroke-width:2px,color:#fff
    classDef server fill:#F5A623,stroke:#D4880F,stroke-width:2px,color:#fff
    class C1 client
    class M1,M2,M3 mcp
    class S1,S2,S3 server
```

### MCP Tools

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

---

## 模式选择指南

```mermaid
flowchart TD
    START["开发 AI Coding Agent?"] -->|"是, 自研 Agent CLI"| Q1
    START -->|"否, 使用第三方 Agent"| Q2

    Q1["使用 Rust 开发?"] -->|"是"| LIB["Library Mode<br/>cargo add lspz"]
    Q1 -->|"否"| Q2

    Q2["Agent 支持 MCP?"] -->|"是, 且只需按需查询"| MCP["MCP Mode<br/>lspz mcp"]
    Q2 -->|"否, 或需要实时诊断推送"| PROXY["Proxy Mode<br/>lspz --backend"]

    LIB -->|"最终方案"| DONE["✅ 零开销, 完全控制"]
    PROXY -->|"立即可用"| DONE2["✅ 透明集成, 无需改代码"]
    MCP -->|"适合实验"| DONE3["✅ 生态兼容, 按需查询"]

    classDef start fill:#4A90E2,stroke:#2E5C8A,stroke-width:2px,color:#fff
    classDef mode fill:#7ED321,stroke:#5BA01A,stroke-width:2px,color:#fff
    classDef done fill:#F5A623,stroke:#D4880F,stroke-width:2px,color:#fff

    class START start
    class LIB,PROXY,MCP mode
    class DONE,DONE2,DONE3 done
```

---

## 兼容性矩阵

| 功能 | Library | Proxy | MCP |
|------|----------|-------|-----|
| 诊断压缩 | ✅ | ✅ | ✅ |
| 自定义传输 | ✅ | ✅ (TCP/WS) | ❌ |
| 运行时配置 | ✅ | ✅ | ⚠️ |
| 热加载 | ✅ | ✅ | ⚠️ |
| 多 server | ✅ | ⚠️ | ✅ |
| 零额外开销 | ✅ | ❌ (进程间) | ❌ (MCP 协议) |

✅ 完全支持 | ⚠️ 部分支持 | ❌ 不支持

---

## Crate 结构

```mermaid
graph TD
    subgraph Crate["lspz (single crate, feature flags)"]
        LIB["src/lib.rs<br/>Public API + module decl"]
        CLI["src/main.rs<br/>CLI entry (feature = cli)"]
        PROXY["proxy.rs<br/>Proxy lifecycle<br/>Message routing<br/>State machine"]
        INTERCEPTOR["interceptors/<br/>mod.rs - Interceptor trait + Chain<br/>8 interceptors (7 compressors + capping)"]
        CODEC["codec/<br/>json_rpc.rs - JSON-RPC 2.0<br/>compact.rs - Compact format<br/>toon.rs - TOON format"]
        TRANSPORT["transport/<br/>stdio.rs - StdioTransport<br/>tcp.rs - TcpTransport<br/>websocket.rs - WsTransport<br/>mock.rs - MockTransport"]
        MCPMOD["mcp/<br/>McpServer (feature = mcp)<br/>LspSession (feature = mcp)<br/>LspPool (feature = mcp)"]
        AGENT["agent_sdk/<br/>AgentHandle (feature = agent-sdk)<br/>AgentPool (feature = agent-sdk)"]
        CONFIG["config.rs + config_watcher.rs<br/>Config + Builder + TOML + hot-reload"]
        METRICS["metrics.rs<br/>MetredInterceptor + MetricsSnapshot"]
        ERROR["error.rs<br/>LspzError enum<br/>thiserror derive"]
    end

    subgraph SSOT["SSOT Output (cargo doc + mdbook)"]
        APIDOC["target/doc/lspz/<br/>from /// code comments"]
        BOOK["docs/book/<br/>from docs/src/**/*.md"]
    end

    CLI -.-> LIB
    MCPMOD -.-> LIB
    AGENT -.-> MCPMOD
    LIB --> PROXY
    LIB --> INTERCEPTOR
    LIB --> CODEC
    LIB --> TRANSPORT
    LIB --> CONFIG
    LIB --> ERROR
    PROXY --> INTERCEPTOR
    PROXY --> CODEC
    PROXY --> TRANSPORT
    PROXY --> CONFIG
    PROXY --> ERROR
    INTERCEPTOR --> CODEC
    INTERCEPTOR --> ERROR
    CODEC --> ERROR

    LIB -.-> APIDOC
    LIB -.-> BOOK

    classDef crate fill:#7ED321,stroke:#5BA01A,stroke-width:2px,color:#fff
    classDef module fill:#4A90E2,stroke:#2E5C8A,stroke-width:2px,color:#fff
    classDef ssot fill:#F5A623,stroke:#D4880F,stroke-width:2px,color:#fff
    classDef optional fill:#9013FE,stroke:#6A0DAD,stroke-width:2px,color:#fff

    class LIB,CLI crate
    class PROXY,INTERCEPTOR,CODEC,TRANSPORT,CONFIG,METRICS,ERROR module
    class MCPMOD,AGENT optional
    class APIDOC,BOOK ssot
```

---

## 已实现扩展

### v0.2 (MCP 集成)
- [x] 实现完整的 MCP Mode
- [x] 添加 MCP tools (get_diagnostics, get_completions, get_symbols)
- [x] 多 LSP server 连接池 (LspPool)

### v0.3 (Agent SDK)
- [x] AgentHandle + AgentPool
- [x] 10 个查询方法 (diagnostics, completions, symbols, hover, references, definition, implementation, typeDefinition, workspace_symbols, workspace_diagnostics)

### v0.4+ (高级特性)
- [x] TCP/WebSocket 传输层
- [x] 动态配置热加载 (notify crate)
- [x] Metrics 和 Tracing (MetredInterceptor)

---

## 参考文档

- [ROADMAP.md](https://github.com/straydragon/lspz/blob/main/ROADMAP.md) - 项目路线图
- [压缩格式规范](./specs/compression-format.md)
- [LSP 兼容性](./specs/lsp-compatibility.md)
- [SSOT 规则](./specs/ssot-rules.md)
- [TOON 格式](./specs/toon-format.md)
