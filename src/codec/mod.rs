//! 消息编解码层。
//!
//! 提供 JSON-RPC 2.0 分帧（[`json_rpc`]）、紧凑诊断格式（[`compact`]）
//! 和 TOON 输出格式（[`toon`]）。

pub mod compact;
pub mod json_rpc;
pub mod toon;
