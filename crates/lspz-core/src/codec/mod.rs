//! Message codec layer.
//!
//! Provides JSON-RPC 2.0 framing ([`json_rpc`]), compact diagnostic format ([`compact`]),
//! and TOON output format ([`toon`]).

pub mod compact;
pub mod json_rpc;
pub mod toon;
