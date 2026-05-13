//! lspz 的统一错误类型。
//!
//! 使用 [`thiserror`] 进行符合人体工程学的错误派生。

use std::io;

/// lspz 代码库的统一错误类型。
#[derive(Debug, thiserror::Error)]
pub enum LspzError {
    #[error("IO 错误: {0}")]
    Io(#[from] io::Error),

    #[error("JSON 解析错误: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("协议错误: {0}")]
    Protocol(String),

    #[error("服务器意外退出")]
    ServerExited,

    #[error("配置错误: {0}")]
    Config(String),
}
