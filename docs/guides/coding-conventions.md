# lspz 编码约定

**版本**: v0.1.0
**最后更新**: 2026-05-09

> **注意**: lspz 项目的通用编码规范定义在项目根目录的 [AGENTS.md](../../AGENTS.md)（SSOT）。本文档仅包含 lspz 特定的约定和补充。

---

## 快速参考

### 通用规范（详见 AGENTS.md）

- [Rust Edition & Toolchain](../../AGENTS.md#rust)
- [命名约定](../../AGENTS.md#命名约定)
- [错误处理](../../AGENTS.md#错误处理)
- [异步代码](../../AGENTS.md#异步代码)
- [日志规范](../../AGENTS.md#日志规范)
- [测试规范](../../AGENTS.md#测试规范)
- [文档规范](../../AGENTS.md#文档规范)
- [安全考虑](../../AGENTS.md#安全考虑)
- [性能考虑](../../AGENTS.md#性能考虑)

**请先阅读 AGENTS.md 了解通用规范。**

---

## lspz 特定约定

### 项目结构约定

```
lspz/
├── Cargo.toml           # single crate, feature flags
├── src/
│   ├── lib.rs           # 公共 API + 模块声明
│   ├── main.rs          # CLI 入口 (feature = "cli")
│   ├── proxy.rs         # Proxy 核心
│   ├── interceptors/    # 所有拦截器实现
│   ├── codec/           # 编解码层（JSON-RPC, 紧凑格式）
│   ├── transport/       # 传输层实现（stdio, TCP, WS）
│   ├── mcp/             # MCP 服务器 (feature = "mcp")
│   ├── agent_sdk/       # Agent SDK (feature = "agent-sdk")
│   ├── config.rs
│   └── error.rs
└── examples/            # 示例代码
```

### 模块组织

- **lib.rs**: 仅重新导出公共 API，不包含实现
- **proxy.rs**: Proxy 核心逻辑，依赖其他模块
- **interceptors/**: 所有拦截器实现
- **codec/**: 编解码层（JSON-RPC, 紧凑格式）
- **transport/**: 传输层实现（stdio, 未来 TCP/WS）

### 公共 API 设计

**原则**: 核心库部分公共 API 必须稳定且易用。

```rust
// ✅ 好: 清晰的构建器模式
let config = Config::builder()
    .backend_cmd("rust-analyzer")
    .enable_diag_compress(true)
    .build();

let proxy = Proxy::new(config).await?;

// ❌ 不好: 复杂的嵌套结构
let proxy = Proxy::new(ProxyConfig {
    backend: BackendConfig {
        command: "rust-analyzer".to_string(),
        args: vec![],
    },
    compression: CompressionConfig {
        enable_diagnostics: true,
        // ...
    },
})?;
```

### 拦截器约定

**拦截器 trait 定义**:

```rust
#[async_trait]
pub trait Interceptor: Send + Sync + AsAny {
    /// 拦截器名称（用于日志和调试）
    fn name(&self) -> &str;

    /// 拦截处理
    async fn intercept(
        &self,
        message: &JsonRpcMessage,
        direction: Direction,
    ) -> Result<Option<JsonRpcMessage>, LspzError>;

    /// 优先级（越小越先执行）
    fn priority(&self) -> i32 {
        0
    }
}
```

**实现拦截器**:

```rust
pub struct DiagnosticsCompressor {
    config: CompressionConfig,
}

#[async_trait]
impl Interceptor for DiagnosticsCompressor {
    fn name(&self) -> &str {
        "diagnostics_compressor"
    }

    fn priority(&self) -> i32 {
        100  // 高优先级
    }

    async fn intercept(
        &self,
        message: &JsonRpcMessage,
        direction: Direction,
    ) -> Result<Option<JsonRpcMessage>, LspzError> {
        // 只处理 Server→Client 的诊断
        if direction != Direction::ServerToClient {
            return Ok(Some(message.clone()));
        }

        if message.method == "textDocument/publishDiagnostics" {
            self.compress_diagnostics(message).await
        } else {
            Ok(Some(message.clone()))
        }
    }
}
```

### 传输层约定

**Transport trait 定义**:

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    /// 发送消息
    async fn send(&self, message: JsonRpcMessage) -> Result<(), LspzError>;

    /// 接收消息
    async fn receive(&self) -> Result<JsonRpcMessage, LspzError>;

    /// 关闭传输
    async fn close(&self) -> Result<(), LspzError>;
}
```

**实现传输层**:

```rust
pub struct StdioTransport {
    reader: BufReader<ChildStdout>,
    writer: BufWriter<ChildStdin>,
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&self, message: JsonRpcMessage) -> Result<(), LspzError> {
        let json = serde_json::to_string(&message)?;
        writeln!(self.writer, "Content-Length: {}", json.len())?;
        writeln!(self.writer)?;
        writeln!(self.writer, "{}", json)?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn receive(&self) -> Result<JsonRpcMessage, LspzError> {
        // 解析 Content-Length
        // 读取 JSON 消息
        // ...
    }

    async fn close(&self) -> Result<(), LspzError> {
        // 清理资源
    }
}
```

### 配置约定

**配置结构体**:

```rust
#[derive(Clone, Debug, Builder)]
pub struct Config {
    /// 后端 LSP 命令
    #[builder(default = "default_backend_cmd")]
    backend_cmd: String,

    /// 是否启用诊断压缩
    #[builder(default)]
    enable_diag_compress: bool,

    /// 日志级别
    #[builder(default = "default_log_level")]
    log_level: Level,
}
```

**从环境变量加载**:

```rust
impl Config {
    pub fn from_env() -> Result<Self, LspzError> {
        let backend_cmd = env::var("LSPZ_BACKEND_CMD")
            .unwrap_or_else(|_| "rust-analyzer".to_string());

        let enable_diag_compress = env::var("LSPZ_ENABLE_DIAG_COMPRESS")
            .map(|s| s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        // ...
    }
}
```

### 错误处理约定

**统一错误类型**:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LspzError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("LSP protocol error: {0}")]
    LspProtocol(String),

    #[error("Compression error: {0}")]
    Compression(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Configuration error: {0}")]
    Config(String),
}
```

**压缩失败降级**:

```rust
// 在 Proxy 核心中
match self.intercept(message, direction) {
    Ok(Some(compressed)) => send(compressed),
    Ok(None) => { /* 消息被丢弃 */ }
    Err(e) => {
        warn!("Compression failed: {}, falling back to transparent forward", e);
        send_original(message);
    }
}
```

### LSP 兼容性约定

**透明转发原则**:

- 非 `textDocument/publishDiagnostics` 消息必须透明转发
- 不得修改 LSP 初始化握手
- 不得修改 server/client capabilities

**能力协商**:

```rust
// lspz 不修改 capabilities，直接转发
async fn handle_initialize(&self, req: Request) -> Result<Response, LspzError> {
    let server_response = self.send_to_backend(req).await?;
    Ok(server_response)  // 直接返回，不修改
}
```

---

## 扩展点预留

### 为未来扩展预留代码

**拦截器动态注册** (v0.3):

```rust
// 当前: 固定列表
let interceptors: Vec<Box<dyn Interceptor>> = vec![
    Box::new(DiagnosticsCompressor::new(config)),
];

// 未来预留: 支持动态注册
// proxy.register_interceptor(Box::new(CustomInterceptor::new()))?;
// proxy.set_interceptor_order(&["diagnostics", "custom"])?;
```

**传输层抽象** (v0.4):

```rust
// 当前: 只有 stdio
let transport = StdioTransport::new(child)?;

// 未来预留: 支持多种传输
// let transport: Box<dyn Transport> = match config.transport_type {
//     TransportType::Stdio => Box::new(StdioTransport::new(child)?),
//     TransportType::Tcp => Box::new(TcpTransport::new(&config.tcp_addr)?),
//     TransportType::WebSocket => Box::new(WebSocketTransport::new(&config.ws_url)?),
// };
```

**配置热加载** (v0.4):

```rust
// 当前: 配置固定
let config = Config::from_env()?;

// 未来预留: 支持热加载
// let hot_reload = HotReloadConfig::new("lspz.toml")?;
// hot_reload.watch(|new_config| {
//     proxy.update_config(new_config);
// });
```

---

## 文档注释模板

### Struct 文档

```rust
/// LSP 代理核心结构体。
///
/// # 示例
///
/// ```no_run
/// use lspz::{Proxy, Config};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = Config::builder()
///         .backend_cmd("rust-analyzer")
///         .enable_diag_compress(true)
///         .build();
///
///     let proxy = Proxy::new(config).await?;
///     proxy.initialize().await?;
///
///     // 使用 proxy...
///     Ok(())
/// }
/// ```
///
/// # 错误
///
/// 如果后端 LSP 启动失败，返回 `LspzError::Io`。
/// 如果 LSP 握手失败，返回 `LspzError::LspProtocol`。
pub struct Proxy {
    // ...
}
```

### Trait 文档

```rust
/// LSP 消息拦截器。
///
/// 拦截器可以修改、丢弃或转发 LSP 消息。
///
/// # 示例
///
/// ```
/// use lspz::interceptors::{Interceptor, Direction};
/// use lspz::{JsonRpcMessage, LspzError};
///
/// struct MyInterceptor;
///
/// #[async_trait::async_trait]
/// impl Interceptor for MyInterceptor {
///     async fn intercept(
///         &self,
///         message: &JsonRpcMessage,
///         direction: Direction,
///     ) -> Result<Option<JsonRpcMessage>, LspzError> {
///         // 实现拦截逻辑
///         Ok(Some(message.clone()))
///     }
/// }
/// ```
pub trait Interceptor: Send + Sync {
    // ...
}
```

---

## 参考文档

- [AGENTS.md](../../AGENTS.md) - 项目通用规范（SSOT）
- [specs/001-tri-modal-architecture.md](../specs/001-tri-modal-architecture.md) - 三模态架构
- [specs/002-compression-format.md](../specs/002-compression-format.md) - 压缩格式规范
- [specs/003-lsp-compatibility.md](../specs/003-lsp-compatibility.md) - LSP 兼容性规范
- [guides/testing-guide.md](testing-guide.md) - 测试指南
