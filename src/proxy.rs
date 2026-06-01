//! LSP 代理状态机和消息循环。
//!
//! [MermaidChart:docs/src/diagrams/proxy-state-machine.mmd]

use std::collections::HashMap;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::select;
use tokio::sync::RwLock;

use crate::codec::compact::CompactDiagnostics;
use crate::codec::json_rpc;
use crate::codec::toon;
use crate::config::{Config, OutputFormat};
use crate::error::LspzError;
use crate::interceptors::workspace_diagnostics::workspace_diagnostics_to_toon;
use crate::interceptors::workspace_symbols::workspace_symbols_to_toon;
use crate::interceptors::{Direction, InterceptorChain};
use crate::transport::Transport;

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
        let mut stdout = tokio::io::stdout();

        // Step 1: read "initialize" Request from client → forward to server
        let init_req = read_stdin_frame(&mut stdin).await?;
        ensure_method(&init_req, "initialize")?;
        self.transport.send(&init_req).await?;
        tracing::debug!("Forwarded 'initialize' request to server");

        // Step 2: read "initialize" Response from server → forward to client
        let init_resp = self.transport.receive().await?;
        stdout.write_all(&init_resp).await?;
        stdout.flush().await?;
        tracing::debug!("Forwarded 'initialize' response to client");

        // Step 3: read "initialized" Notification from client → forward to server
        let init_not = read_stdin_frame(&mut stdin).await?;
        ensure_method(&init_not, "initialized")?;
        self.transport.send(&init_not).await?;
        tracing::debug!("Forwarded 'initialized' notification to server");

        Ok(())
    }

    // ─── Message Loop ──────────────────────────────────────────────────

    /// Main message loop using [`tokio::select!`].
    ///
    /// - Client → Server: transparent forward
    /// - Server → Client: interceptor chain → forward
    async fn message_loop(&mut self) -> Result<(), LspzError> {
        let mut stdin = BufReader::new(tokio::io::stdin());
        let mut stdout = tokio::io::stdout();

        loop {
            select! {
                // ── Client → Server (transparent passthrough) ──────
                client_msg = read_stdin_frame(&mut stdin) => {
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
                        self.transport.send(&msg_bytes).await?;
                        self.state = State::ShuttingDown;
                        // Forward server response then exit
                        let resp = self.transport.receive().await?;
                        stdout.write_all(&resp).await?;
                        stdout.flush().await?;
                        self.state = State::Exited;
                        return Ok(());
                    }

                    // Track request ID → method mapping for response interception
                    track_pending_request(&msg_bytes, &mut self.pending_requests);

                    self.transport.send(&msg_bytes).await?;
                }

                // ── Server → Client (through interceptor chain) ────
                server_msg = self.transport.receive() => {
                    let msg_bytes = match server_msg {
                        Ok(bytes) => bytes,
                        Err(LspzError::ServerExited) => {
                            tracing::warn!("Server exited unexpectedly");
                            self.state = State::Exited;
                            return Err(LspzError::ServerExited);
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Error reading from server");
                            return Err(e);
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
            return self.process_notification(method, &json_val, raw).await;
        }

        // Response (no `method`): look up from pending requests
        if json_val.get("id").is_some() {
            return self.process_response(&json_val, raw).await;
        }

        raw.to_vec()
    }

    /// Process a notification or server→client request through the interceptor chain.
    async fn process_notification(
        &self,
        method: &str,
        json_val: &serde_json::Value,
        raw: &[u8],
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
            Err(_) => return raw.to_vec(), // Fail-open
        };

        // TOON mode: convert compact params to TOON text
        if self.config.read().await.output_format == OutputFormat::Toon {
            return self.toon_output(method, &transformed, raw);
        }

        // JSON mode: reconstruct message with transformed params (default)
        let mut obj = match json_val.as_object() {
            Some(o) => o.clone(),
            None => return raw.to_vec(),
        };
        obj.insert("params".into(), transformed);

        let new_val = serde_json::Value::Object(obj);
        match json_rpc::serialize_frame(&new_val) {
            Ok(bytes) => bytes,
            Err(_) => raw.to_vec(),
        }
    }

    /// Process a server→client response (no method, has id).
    ///
    /// Looks up the original request method from `pending_requests`,
    /// passes the response `result` through the interceptor chain,
    /// then reconstructs the response.
    async fn process_response(&mut self, json_val: &serde_json::Value, raw: &[u8]) -> Vec<u8> {
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
            Err(_) => return raw.to_vec(), // Fail-open
        };

        // Reconstruct response with transformed result
        let mut obj = match json_val.as_object() {
            Some(o) => o.clone(),
            None => return raw.to_vec(),
        };
        obj.insert("result".into(), transformed);

        let new_val = serde_json::Value::Object(obj);
        match json_rpc::serialize_frame(&new_val) {
            Ok(bytes) => bytes,
            Err(_) => raw.to_vec(),
        }
    }

    /// Convert transformed params to TOON format and wrap in JSON-RPC.
    fn toon_output(&self, method: &str, params: &serde_json::Value, raw: &[u8]) -> Vec<u8> {
        let toon_text = match method {
            "textDocument/publishDiagnostics" => {
                // Deserialize compact Value back to typed struct
                match serde_json::from_value::<CompactDiagnostics>(params.clone()) {
                    Ok(compact) => toon::diagnostics_to_toon(&compact),
                    Err(_) => return raw.to_vec(),
                }
            }
            "textDocument/completion" => match toon::completions_to_toon(params) {
                Ok(t) => t,
                Err(_) => return raw.to_vec(),
            },
            "textDocument/hover" => match toon::hover_to_toon(params) {
                Ok(t) => t,
                Err(_) => return raw.to_vec(),
            },
            "textDocument/documentSymbol" => match toon::symbols_to_toon(params) {
                Ok(t) => t,
                Err(_) => return raw.to_vec(),
            },
            "textDocument/references"
            | "textDocument/definition"
            | "textDocument/implementation"
            | "textDocument/typeDefinition" => match toon::locations_to_toon(params) {
                Ok(t) => t,
                Err(_) => return raw.to_vec(),
            },
            "workspace/symbol" => match workspace_symbols_to_toon(params) {
                Ok(t) => t,
                Err(_) => return raw.to_vec(),
            },
            "workspace/diagnostic" => match workspace_diagnostics_to_toon(params) {
                Ok(t) => t,
                Err(_) => return raw.to_vec(),
            },
            // Unknown method → passthrough
            _ => return raw.to_vec(),
        };

        // Wrap TOON text in a JSON-RPC notification
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

// ─── I/O Helpers ────────────────────────────────────────────────────────────

/// Read one complete Content-Length framed message from stdin.
async fn read_stdin_frame(reader: &mut BufReader<tokio::io::Stdin>) -> Result<Vec<u8>, LspzError> {
    let mut header = String::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                LspzError::ServerExited
            } else {
                LspzError::Io(e)
            }
        })?;

        if n == 0 {
            return Err(LspzError::ServerExited);
        }

        header.push_str(&line);

        // A blank line marks the end of headers
        if line == "\r\n" || line == "\n" {
            break;
        }
    }

    let content_length = crate::transport::framing::parse_content_length(&header)?;

    let mut body = vec![0u8; content_length as usize];
    reader.read_exact(&mut body).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            LspzError::ServerExited
        } else {
            LspzError::Io(e)
        }
    })?;

    Ok([header.as_bytes(), &body].concat())
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
}
