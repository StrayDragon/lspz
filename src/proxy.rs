//! LSP 代理状态机和消息循环。
//!
//! 状态：Created → Initializing → Ready → ShuttingDown → Exited。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncWriteExt, BufReader};
use tokio::select;
use tokio::sync::RwLock;
use tokio::time::timeout;

use crate::codec::compact::CompactDiagnostics;
use crate::codec::json_rpc;
use crate::codec::toon;
use crate::config::{Config, OutputFormat};
use crate::error::LspzError;
use crate::interceptors::workspace_diagnostics::workspace_diagnostics_to_toon;
use crate::interceptors::workspace_symbols::workspace_symbols_to_toon;
use crate::interceptors::{Direction, InterceptorChain};
use crate::transport::Transport;
use crate::transport::framing::{self, FrameState};

/// Default timeout waiting for a server frame during handshake / message loop.
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(30);

/// 代理状态机状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// 调用 [`Proxy::start`] 之前的初始状态。
    Created,
    /// 正在执行 LSP initialize/initialized 握手。
    Initializing,
    /// 握手完成，消息循环运行中。
    Ready,
    /// 已请求关闭，正在排空剩余消息。
    ShuttingDown,
    /// 完全退出。
    Exited,
}

/// lspz 代理。
///
/// 将客户端 I/O（stdin/stdout）与服务端 [`Transport`]
/// 和 [`InterceptorChain`] 组合，用于服务端→客户端消息转换。
pub struct Proxy {
    config: Arc<RwLock<Config>>,
    state: State,
    transport: Box<dyn Transport>,
    interceptor_chain: InterceptorChain,
    /// Tracks in-flight request IDs to their method for response interception.
    pending_requests: HashMap<u64, String>,
    /// Holds the config watcher alive so hot-reload keeps working.
    _config_watcher: Option<crate::config_watcher::ConfigWatcher>,
}

impl Proxy {
    /// 创建新的 [`Proxy`]。
    pub fn new(
        config: Arc<RwLock<Config>>,
        transport: Box<dyn Transport>,
        interceptor_chain: InterceptorChain,
    ) -> Self {
        Self {
            config,
            state: State::Created,
            transport,
            interceptor_chain,
            pending_requests: HashMap::new(),
            _config_watcher: None,
        }
    }

    /// 返回当前 [`State`]。
    pub fn state(&self) -> State {
        self.state
    }

    /// Set the config watcher for hot-reload support.
    pub fn set_config_watcher(&mut self, watcher: crate::config_watcher::ConfigWatcher) {
        self._config_watcher = Some(watcher);
    }

    /// Get a reference to the shared config.
    pub fn shared_config(&self) -> Arc<RwLock<Config>> {
        self.config.clone()
    }

    /// 启动代理：握手 → 消息循环。
    pub async fn start(&mut self) -> Result<(), LspzError> {
        self.state = State::Initializing;
        tracing::info!("Proxy starting (handshake)");

        self.perform_handshake().await?;

        self.state = State::Ready;
        tracing::info!("Proxy ready, entering message loop");
        self.message_loop().await
    }

    // ─── Handshake ─────────────────────────────────────────────────────

    /// LSP initialize/initialized 握手。
    async fn perform_handshake(&mut self) -> Result<(), LspzError> {
        let mut stdin = BufReader::new(tokio::io::stdin());
        let mut stdin_state = FrameState::new();
        let mut stdout = tokio::io::stdout();

        // Step 1: read "initialize" Request from client → forward to server
        let init_req = framing::read_frame_with_state(&mut stdin, &mut stdin_state).await?;
        ensure_method(&init_req, "initialize")?;
        let init_id = extract_request_id(&init_req)
            .ok_or_else(|| LspzError::Protocol("initialize request missing numeric id".into()))?;
        self.transport.send(&init_req).await?;
        tracing::debug!(id = init_id, "Forwarded 'initialize' request to server");

        // Step 2: wait for matching initialize Response; forward interleaved notifications
        let init_resp = self
            .recv_response_by_id(init_id, &mut stdout, "initialize")
            .await?;
        stdout.write_all(&init_resp).await?;
        stdout.flush().await?;
        tracing::debug!("Forwarded 'initialize' response to client");

        // Step 3: read "initialized" Notification from client → forward to server
        let init_not = framing::read_frame_with_state(&mut stdin, &mut stdin_state).await?;
        ensure_method(&init_not, "initialized")?;
        self.transport.send(&init_not).await?;
        tracing::debug!("Forwarded 'initialized' notification to server");

        Ok(())
    }

    /// Receive server frames until a response with `expected_id` arrives.
    /// Non-matching notifications (and other messages) are forwarded to `stdout`.
    async fn recv_response_by_id(
        &mut self,
        expected_id: u64,
        stdout: &mut tokio::io::Stdout,
        context: &str,
    ) -> Result<Vec<u8>, LspzError> {
        loop {
            let raw = self.recv_server_frame(context).await?;
            if frame_response_id(&raw) == Some(expected_id) {
                return Ok(raw);
            }
            // Interleaved notification / unrelated message → forward as-is
            tracing::debug!(
                expected_id,
                "Forwarding interleaved server message during {context}"
            );
            stdout.write_all(&raw).await?;
            stdout.flush().await?;
        }
    }

    async fn recv_server_frame(&mut self, context: &str) -> Result<Vec<u8>, LspzError> {
        match timeout(RECEIVE_TIMEOUT, self.transport.receive()).await {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                tracing::error!(context, "Timeout waiting for server frame");
                Err(LspzError::Timeout(format!(
                    "timeout waiting for server frame during {context}"
                )))
            }
        }
    }

    // ─── Message Loop ──────────────────────────────────────────────────

    /// Main message loop using [`tokio::select!`].
    ///
    /// Client and server reads use cancel-safe [`FrameState`] / transport frame
    /// state so a cancelled branch does not desync the stream.
    async fn message_loop(&mut self) -> Result<(), LspzError> {
        let mut stdin = BufReader::new(tokio::io::stdin());
        let mut stdin_state = FrameState::new();
        let mut stdout = tokio::io::stdout();

        loop {
            select! {
                // ── Client → Server (transparent passthrough) ──────
                client_msg = framing::read_frame_with_state(&mut stdin, &mut stdin_state) => {
                    let msg_bytes = match client_msg {
                        Ok(bytes) => bytes,
                        Err(LspzError::ServerExited) => {
                            tracing::info!("Client stdin closed, shutting down");
                            self.state = State::Exited;
                            return Ok(());
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Error reading from client");
                            return Err(e);
                        }
                    };

                    // Check for shutdown request
                    if extract_method(&msg_bytes).is_ok_and(|m| m == "shutdown") {
                        tracing::info!("Received 'shutdown' from client");
                        let shutdown_id = extract_request_id(&msg_bytes);
                        self.transport.send(&msg_bytes).await?;
                        self.state = State::ShuttingDown;
                        if let Some(id) = shutdown_id {
                            let resp = self
                                .recv_response_by_id(id, &mut stdout, "shutdown")
                                .await?;
                            stdout.write_all(&resp).await?;
                            stdout.flush().await?;
                        } else {
                            // Fall back: next frame (legacy servers)
                            let resp = self.recv_server_frame("shutdown").await?;
                            stdout.write_all(&resp).await?;
                            stdout.flush().await?;
                        }
                        self.state = State::Exited;
                        return Ok(());
                    }

                    // Track request ID → method mapping for response interception
                    track_pending_request(&msg_bytes, &mut self.pending_requests);

                    self.transport.send(&msg_bytes).await?;
                }

                // ── Server → Client (through interceptor chain) ────
                server_msg = timeout(RECEIVE_TIMEOUT, self.transport.receive()) => {
                    let msg_bytes = match server_msg {
                        Ok(Ok(bytes)) => bytes,
                        Ok(Err(LspzError::ServerExited)) => {
                            tracing::warn!("Server exited unexpectedly");
                            self.state = State::Exited;
                            return Err(LspzError::ServerExited);
                        }
                        Ok(Err(e)) => {
                            tracing::error!(error = %e, "Error reading from server");
                            return Err(e);
                        }
                        Err(_) => {
                            tracing::error!("Timeout waiting for server message in loop");
                            return Err(LspzError::Timeout(
                                "timeout waiting for server frame in message loop".into(),
                            ));
                        }
                    };

                    let processed = self.process_server_message(&msg_bytes).await;
                    if !processed.is_empty() {
                        stdout.write_all(&processed).await?;
                        stdout.flush().await?;
                    }
                }
            }
        }
    }

    /// Process a raw server→client message through the interceptor chain.
    ///
    /// Supports both notifications (with `method`) and responses (with `id`).
    /// Returns the (possibly transformed) frame bytes, or empty if dropped.
    /// Always succeeds: on error, returns the original raw bytes (fail-open).
    async fn process_server_message(&mut self, raw: &[u8]) -> Vec<u8> {
        let format = self.config.read().await.output_format;
        if format == OutputFormat::Passthrough {
            return raw.to_vec();
        }

        let (frame, _) = match json_rpc::parse_frame(raw) {
            Ok(Some(result)) => result,
            _ => return raw.to_vec(),
        };
        let json_val: serde_json::Value = match serde_json::from_slice(&frame.body) {
            Ok(v) => v,
            Err(_) => return raw.to_vec(),
        };

        // Notification or server→client request: has `method` field
        if let Some(method) = json_val.get("method").and_then(|v| v.as_str()) {
            return self
                .process_notification(method, &json_val, raw, format)
                .await;
        }

        // Response (no `method`): look up from pending requests
        if json_val.get("id").is_some() {
            return self.process_response(&json_val, raw, format).await;
        }

        raw.to_vec()
    }

    /// Process a notification or server→client request through the interceptor chain.
    async fn process_notification(
        &self,
        method: &str,
        json_val: &serde_json::Value,
        raw: &[u8],
        format: OutputFormat,
    ) -> Vec<u8> {
        let params = json_val
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let transformed = match self
            .interceptor_chain
            .process(method, params, Direction::ServerToClient)
            .await
        {
            Ok(Some(p)) => p,
            Ok(None) => return Vec::new(), // Dropped by interceptor
            Err(e) => {
                tracing::warn!(error = %e, method, "Interceptor failed, fail-open");
                return raw.to_vec();
            }
        };

        if format == OutputFormat::Toon {
            return self.toon_notification(method, &transformed, raw);
        }

        // JSON mode: reconstruct message with transformed params
        let mut obj = match json_val.as_object() {
            Some(o) => o.clone(),
            None => return raw.to_vec(),
        };
        obj.insert("params".into(), transformed);

        let new_val = serde_json::Value::Object(obj);
        match json_rpc::serialize_frame(&new_val) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to serialize notification, fail-open");
                raw.to_vec()
            }
        }
    }

    /// Process a server→client response (no method, has id).
    async fn process_response(
        &mut self,
        json_val: &serde_json::Value,
        raw: &[u8],
        format: OutputFormat,
    ) -> Vec<u8> {
        let id = match json_val.get("id").and_then(|v| v.as_u64()) {
            Some(id) => id,
            None => return raw.to_vec(),
        };

        // Look up and remove the request method; if unknown, pass through
        let method = match self.pending_requests.remove(&id) {
            Some(m) => m,
            None => return raw.to_vec(),
        };

        // Skip error responses
        if json_val.get("error").is_some() {
            return raw.to_vec();
        }

        // Extract result as params
        let params = json_val
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let transformed = match self
            .interceptor_chain
            .process(&method, params, Direction::ServerToClient)
            .await
        {
            Ok(Some(p)) => p,
            Ok(None) => return Vec::new(), // Dropped by interceptor
            Err(e) => {
                tracing::warn!(error = %e, method = %method, "Interceptor failed, fail-open");
                return raw.to_vec();
            }
        };

        if format == OutputFormat::Toon {
            return self.toon_response(id, &method, &transformed, raw);
        }

        // Reconstruct response with transformed result
        let mut obj = match json_val.as_object() {
            Some(o) => o.clone(),
            None => return raw.to_vec(),
        };
        obj.insert("result".into(), transformed);

        let new_val = serde_json::Value::Object(obj);
        match json_rpc::serialize_frame(&new_val) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to serialize response, fail-open");
                raw.to_vec()
            }
        }
    }

    /// Convert transformed params to TOON and wrap as a JSON-RPC notification.
    fn toon_notification(&self, method: &str, params: &serde_json::Value, raw: &[u8]) -> Vec<u8> {
        let toon_text = match encode_toon(method, params) {
            Some(t) => t,
            None => return raw.to_vec(),
        };

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": {
                "format": "toon",
                "text": toon_text,
            }
        });

        match json_rpc::serialize_frame(&msg) {
            Ok(bytes) => bytes,
            Err(_) => raw.to_vec(),
        }
    }

    /// Convert transformed result to TOON and wrap as a JSON-RPC response (keeps `id`).
    fn toon_response(
        &self,
        id: u64,
        method: &str,
        params: &serde_json::Value,
        raw: &[u8],
    ) -> Vec<u8> {
        let toon_text = match encode_toon(method, params) {
            Some(t) => t,
            None => return raw.to_vec(),
        };

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "format": "toon",
                "text": toon_text,
            }
        });

        match json_rpc::serialize_frame(&msg) {
            Ok(bytes) => bytes,
            Err(_) => raw.to_vec(),
        }
    }
}

/// Encode interceptor output to TOON text for a known method.
fn encode_toon(method: &str, params: &serde_json::Value) -> Option<String> {
    match method {
        "textDocument/publishDiagnostics" => {
            match serde_json::from_value::<CompactDiagnostics>(params.clone()) {
                Ok(compact) => Some(toon::diagnostics_to_toon(&compact)),
                Err(_) => None,
            }
        }
        "textDocument/completion" => toon::completions_to_toon(params).ok(),
        "textDocument/hover" => toon::hover_to_toon(params).ok(),
        "textDocument/documentSymbol" => toon::symbols_to_toon(params).ok(),
        "textDocument/references"
        | "textDocument/definition"
        | "textDocument/implementation"
        | "textDocument/typeDefinition" => toon::locations_to_toon(params).ok(),
        "workspace/symbol" => workspace_symbols_to_toon(params).ok(),
        "workspace/diagnostic" => workspace_diagnostics_to_toon(params).ok(),
        _ => None,
    }
}

/// Track a client→server request ID → method mapping for response interception.
fn track_pending_request(raw: &[u8], pending: &mut HashMap<u64, String>) {
    let (frame, _) = match json_rpc::parse_frame(raw) {
        Ok(Some(f)) => f,
        _ => return,
    };
    let val: serde_json::Value = match serde_json::from_slice(&frame.body) {
        Ok(v) => v,
        _ => return,
    };
    // Only track requests (must have both id and method)
    let id = match val.get("id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return,
    };
    let method = match val.get("method").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => return,
    };
    pending.insert(id, method.to_string());
}

/// Extract the LSP method name from a raw framed message.
fn extract_method(raw: &[u8]) -> Result<String, LspzError> {
    let (frame, _) = json_rpc::parse_frame(raw)?
        .ok_or_else(|| LspzError::Protocol("incomplete frame".into()))?;
    let val: serde_json::Value = serde_json::from_slice(&frame.body)?;
    val.get("method")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| LspzError::Protocol("no 'method' field in message".into()))
}

/// Extract a numeric JSON-RPC request id from a framed message.
fn extract_request_id(raw: &[u8]) -> Option<u64> {
    let (frame, _) = json_rpc::parse_frame(raw).ok().flatten()?;
    let val: serde_json::Value = serde_json::from_slice(&frame.body).ok()?;
    val.get("id").and_then(|v| v.as_u64())
}

/// Extract response id from a framed message (responses have `id` and no `method`).
fn frame_response_id(raw: &[u8]) -> Option<u64> {
    let (frame, _) = json_rpc::parse_frame(raw).ok().flatten()?;
    let val: serde_json::Value = serde_json::from_slice(&frame.body).ok()?;
    if val.get("method").is_some() {
        return None;
    }
    val.get("id").and_then(|v| v.as_u64())
}

/// Ensure a raw message has the expected method name.
fn ensure_method(raw: &[u8], expected: &str) -> Result<(), LspzError> {
    let method = extract_method(raw)?;
    if method != expected {
        return Err(LspzError::Protocol(format!(
            "expected '{expected}', got '{method}'"
        )));
    }
    Ok(())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::json_rpc::LspMessage;
    use crate::transport::framing::parse_content_length;

    #[test]
    fn test_extract_method() {
        let msg = LspMessage::Notification {
            method: "textDocument/publishDiagnostics".into(),
            params: serde_json::Value::Null,
        };
        let bytes = msg.to_bytes().unwrap();
        assert_eq!(
            extract_method(&bytes).unwrap(),
            "textDocument/publishDiagnostics"
        );
    }

    #[test]
    fn test_ensure_method_ok() {
        let msg = LspMessage::Request {
            id: 1,
            method: "initialize".into(),
            params: serde_json::Value::Null,
        };
        let bytes = msg.to_bytes().unwrap();
        assert!(ensure_method(&bytes, "initialize").is_ok());
    }

    #[test]
    fn test_ensure_method_err() {
        let msg = LspMessage::Request {
            id: 1,
            method: "shutdown".into(),
            params: serde_json::Value::Null,
        };
        let bytes = msg.to_bytes().unwrap();
        assert!(ensure_method(&bytes, "initialize").is_err());
    }

    #[test]
    fn test_parse_content_length() {
        let header = "Content-Length: 42\r\n\r\n";
        assert_eq!(parse_content_length(header).unwrap(), 42);
    }

    #[test]
    fn test_missing_content_length() {
        let header = "\r\n\r\n";
        assert!(parse_content_length(header).is_err());
    }

    #[test]
    fn test_invalid_content_length_value() {
        let header = "Content-Length: abc\r\n\r\n";
        assert!(parse_content_length(header).is_err());
    }

    #[test]
    fn test_extract_request_id_and_frame_response_id() {
        let req = LspMessage::Request {
            id: 7,
            method: "textDocument/completion".into(),
            params: serde_json::Value::Null,
        };
        let req_bytes = req.to_bytes().unwrap();
        assert_eq!(extract_request_id(&req_bytes), Some(7));

        let resp = serde_json::json!({"jsonrpc":"2.0","id":7,"result":{}});
        let resp_bytes = json_rpc::serialize_frame(&resp).unwrap();
        assert_eq!(frame_response_id(&resp_bytes), Some(7));

        let note = LspMessage::Notification {
            method: "window/logMessage".into(),
            params: serde_json::json!({"type": 3, "message": "hi"}),
        };
        let note_bytes = note.to_bytes().unwrap();
        assert_eq!(frame_response_id(&note_bytes), None);
    }

    #[tokio::test]
    async fn test_passthrough_skips_compression() {
        use crate::interceptors::default_interceptors;
        use crate::transport::mock::MockTransport;

        let config = Arc::new(RwLock::new(
            Config::builder()
                .backend_cmd("mock")
                .output_format(OutputFormat::Passthrough)
                .build()
                .unwrap(),
        ));
        let chain = InterceptorChain::new(default_interceptors(), config.clone());
        let mut proxy = Proxy::new(config, Box::new(MockTransport::new()), chain);

        let note = LspMessage::Notification {
            method: "textDocument/publishDiagnostics".into(),
            params: serde_json::json!({
                "uri": "file:///t.rs",
                "diagnostics": [{
                    "range": {"start":{"line":0,"character":0},"end":{"line":0,"character":1}},
                    "message": "unused",
                    "severity": 2
                }]
            }),
        };
        let raw = note.to_bytes().unwrap();
        let out = proxy.process_server_message(&raw).await;
        assert_eq!(out, raw);
    }

    #[tokio::test]
    async fn test_toon_response_preserves_id() {
        use crate::interceptors::default_interceptors;
        use crate::transport::mock::MockTransport;

        let config = Arc::new(RwLock::new(
            Config::builder()
                .backend_cmd("mock")
                .output_format(OutputFormat::Toon)
                .build()
                .unwrap(),
        ));
        let chain = InterceptorChain::new(default_interceptors(), config.clone());
        let mut proxy = Proxy::new(config, Box::new(MockTransport::new()), chain);
        proxy
            .pending_requests
            .insert(7, "textDocument/completion".into());

        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "result": {
                "isIncomplete": false,
                "items": [{"label": "foo", "kind": 1}]
            }
        });
        let raw = json_rpc::serialize_frame(&resp).unwrap();
        let out = proxy.process_server_message(&raw).await;
        let (frame, _) = json_rpc::parse_frame(&out).unwrap().unwrap();
        let val: serde_json::Value = serde_json::from_slice(&frame.body).unwrap();
        assert_eq!(val["id"], 7);
        assert_eq!(val["result"]["format"], "toon");
        assert!(val["result"]["text"].as_str().unwrap().contains("foo"));
    }
}
