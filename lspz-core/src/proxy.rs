//! LSP proxy state machine and message loop.
//!
//! [MermaidChart:./docs/mmd/proxy-state-machine.mmd]

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::select;

use crate::codec::json_rpc;
use crate::config::Config;
use crate::error::LspzError;
use crate::interceptors::{Direction, InterceptorChain};
use crate::transport::Transport;

/// Proxy state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Initial state before [`Proxy::start`] is called.
    Created,
    /// Performing LSP initialize/initialized handshake.
    Initializing,
    /// Handshake complete, message loop running.
    Ready,
    /// Shutdown requested, draining remaining messages.
    ShuttingDown,
    /// Fully exited.
    Exited,
}

/// The lspz proxy.
///
/// Combines a client-side I/O (stdin/stdout) with a server-side [`Transport`]
/// and an [`InterceptorChain`] for Server→Client message transformation.
pub struct Proxy {
    _config: Config,
    state: State,
    transport: Box<dyn Transport>,
    interceptor_chain: InterceptorChain,
}

impl Proxy {
    /// Create a new [`Proxy`].
    pub fn new(
        config: Config,
        transport: Box<dyn Transport>,
        interceptor_chain: InterceptorChain,
    ) -> Self {
        Self {
            _config: config,
            state: State::Created,
            transport,
            interceptor_chain,
        }
    }

    /// Returns the current [`State`].
    pub fn state(&self) -> State {
        self.state
    }

    /// Start the proxy: handshake → message loop.
    pub async fn start(&mut self) -> Result<(), LspzError> {
        self.state = State::Initializing;
        tracing::info!("Proxy starting (handshake)");

        self.perform_handshake().await?;

        self.state = State::Ready;
        tracing::info!("Proxy ready, entering message loop");
        self.message_loop().await
    }

    // ─── Handshake ─────────────────────────────────────────────────────

    /// LSP initialize/initialized handshake.
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
    /// Returns the (possibly transformed) frame bytes, or empty if dropped.
    /// Always succeeds: on error, returns the original raw bytes (fail-open).
    async fn process_server_message(&self, raw: &[u8]) -> Vec<u8> {
        let (frame, _) = match json_rpc::parse_frame(raw) {
            Ok(Some(result)) => result,
            _ => return raw.to_vec(),
        };
        let json_val: serde_json::Value = match serde_json::from_slice(&frame.body) {
            Ok(v) => v,
            Err(_) => return raw.to_vec(),
        };

        let method = match json_val.get("method").and_then(|v| v.as_str()) {
            Some(m) => m,
            None => return raw.to_vec(), // Response without method → no interception
        };

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

        // Reconstruct the message with transformed params
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

    let content_length = parse_content_length(&header)?;

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

/// Parse Content-Length from an LSP header.
fn parse_content_length(header: &str) -> Result<u64, LspzError> {
    for line in header.lines() {
        let line = line.trim();
        if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
            let value = value.trim();
            return value
                .parse::<u64>()
                .map_err(|_| LspzError::Protocol(format!("invalid Content-Length: {value}")));
        }
    }
    Err(LspzError::Protocol("missing Content-Length header".into()))
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
