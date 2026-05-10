//! LSP session — manages a single LSP server connection.
//!
//! Wraps a [`Transport`] with initialize handshake and
//! request/response/notification message exchange.

use std::sync::atomic::{AtomicI64, Ordering};

use lspz_core::StdioTransport;
use lspz_core::Transport;
use lspz_core::codec::json_rpc::LspMessage;
use serde_json::Value;

/// A connected LSP server session.
///
/// Usage:
/// ```ignore
/// let mut session = LspSession::spawn("rust-analyzer").await?;
/// session.initialize().await?;
/// let result = session.send_request("textDocument/completion", params).await?;
/// ```
pub struct LspSession {
    transport: Box<dyn Transport>,
    next_id: AtomicI64,
}

impl LspSession {
    /// Spawn an LSP server and return an uninitialized session.
    pub fn spawn(cmd: &str) -> Result<Self, anyhow::Error> {
        let transport = StdioTransport::spawn(cmd)?;
        Ok(Self {
            transport: Box::new(transport),
            next_id: AtomicI64::new(1),
        })
    }

    /// Create a session with a pre-constructed transport.
    ///
    /// Useful for testing with mock transports.
    pub fn with_transport(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            next_id: AtomicI64::new(1),
        }
    }

    /// Perform the LSP initialize/initialized handshake.
    ///
    /// Sends `initialize` with basic capabilities, waits for the response,
    /// then sends `initialized` notification.
    pub async fn initialize(&mut self) -> Result<Value, anyhow::Error> {
        let params = serde_json::json!({
            "processId": null,
            "capabilities": {},
            "rootUri": null,
            "workspaceFolders": null,
        });
        let result = self.send_request("initialize", params).await?;
        self.send_notification("initialized", serde_json::json!({}))
            .await?;
        Ok(result)
    }

    /// Send a request and wait for the matching response.
    pub async fn send_request(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, anyhow::Error> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = LspMessage::Request {
            id,
            method: method.into(),
            params,
        };
        let frame = msg.to_bytes()?;
        self.transport.send(&frame).await?;

        // Read frames until we get the matching response
        loop {
            let raw = self.transport.receive().await?;
            let parsed = LspMessage::from_frame_bytes(&raw)?;
            match parsed {
                LspMessage::Response {
                    id: rid,
                    result,
                    error,
                } if rid == id => {
                    if let Some(err) = error {
                        anyhow::bail!("LSP error {}: {}", err.code, err.message);
                    }
                    return Ok(result.unwrap_or(Value::Null));
                }
                LspMessage::Response {
                    id: rid, ref error, ..
                } => {
                    tracing::warn!(
                        "Ignoring response for id {} (waiting for {}): {:?}",
                        rid,
                        id,
                        error
                    );
                }
                LspMessage::Notification { method: m, .. } => {
                    tracing::trace!("Buffered notification: {}", m);
                }
                LspMessage::Request { method: m, .. } => {
                    tracing::trace!("Ignored request during wait: {}", m);
                }
            }
        }
    }

    /// Send a notification (fire-and-forget).
    pub async fn send_notification(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<(), anyhow::Error> {
        let msg = LspMessage::Notification {
            method: method.into(),
            params,
        };
        let frame = msg.to_bytes()?;
        self.transport.send(&frame).await?;
        Ok(())
    }

    /// Read and discard frames until a notification with the given method arrives.
    ///
    /// Returns the notification params.
    pub async fn wait_for_notification(&mut self, method: &str) -> Result<Value, anyhow::Error> {
        loop {
            let raw = self.transport.receive().await?;
            let parsed = LspMessage::from_frame_bytes(&raw)?;
            match parsed {
                LspMessage::Notification { method: m, params } if m == method => return Ok(params),
                LspMessage::Notification { method: m, .. } => {
                    tracing::trace!("Skipping notification: {}", m);
                }
                LspMessage::Response { id, ref result, .. } => {
                    tracing::trace!("Skipping response id={}: {:?}", id, result);
                }
                LspMessage::Request { method: m, .. } => {
                    tracing::trace!("Skipping request: {}", m);
                }
            }
        }
    }

    /// Check if the child process has exited.
    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, anyhow::Error> {
        Ok(self.transport.try_wait()?)
    }
}
