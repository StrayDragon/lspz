//! 传输层抽象。
//!
//! 定义所有传输层必须实现的 I/O trait。
//!
//! ## 可用的传输层
//!
//! | Transport | 协议 | Feature Flag | 状态 |
//! |-----------|----------|-------------|--------|
//! | [`stdio::StdioTransport`] | 子进程 stdio | 始终 | ✅ |
//! | [`tcp::TcpTransport`] | TCP 套接字 | 始终 | ✅ |
//! | [`websocket::WsTransport`] | WebSocket | `transport-websocket` | ✅ |
//! | [`mock::MockTransport`] | 内存 FIFO | 始终（测试） | ✅ |
//!
//! ## Architecture（架构）
//!
//! [MermaidChart:docs/src/diagrams/transport-architecture.mmd]

pub(crate) mod framing;
pub mod mock;
pub mod stdio;
pub mod tcp;

/// 此模块仅在启用 `transport-websocket` 功能时可用。
#[cfg(feature = "transport-websocket")]
pub mod websocket;

use std::process::ExitStatus;

use crate::error::LspzError;

/// LSP 通信的抽象 I/O 通道。
///
/// 所有 LSP 消息 I/O（无论传输协议如何）都由此 trait 定义。
/// 实现内部处理 Content-Length 分帧，并暴露原始的分帧字节。
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// 接收一条原始 LSP 消息（Content-Length 分帧）。
    async fn receive(&mut self) -> Result<Vec<u8>, LspzError>;

    /// 向 LSP 服务器/客户端发送原始字节。
    async fn send(&mut self, data: &[u8]) -> Result<(), LspzError>;

    /// 检查底层进程是否已退出。
    ///
    /// 对于非进程传输层（mock、TCP、WebSocket），默认返回 `Ok(None)`。
    fn try_wait(&mut self) -> Result<Option<ExitStatus>, LspzError> {
        Ok(None)
    }
}
