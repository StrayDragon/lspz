//! LSP session — manages a single LSP server connection.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use crate::StdioTransport;
use crate::Transport;
use crate::codec::json_rpc::LspMessage;
use serde_json::Value;

/// Parameters for [`LspSession::initialize`].
#[derive(Default)]
pub struct InitializeParams {
    /// Workspace root URI. Auto-prefixed with `file://` if not already present.
    pub root_uri: Option<String>,
}

/// A connected LSP server session.
pub struct LspSession {
    transport: Box<dyn Transport>,
    next_id: AtomicI64,
}

impl LspSession {
    /// Spawn an LSP server and return an uninitialized session.
    pub fn spawn(cmd: &str) -> Result<Self, anyhow::Error> {
        let transport = StdioTransport::spawn(cmd, &[])?;
        Ok(Self {
            transport: Box::new(transport),
            next_id: AtomicI64::new(1),
        })
    }

    /// Create a session with a pre-constructed transport.
    pub fn with_transport(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            next_id: AtomicI64::new(1),
        }
    }

    /// Perform the LSP initialize/initialized handshake.
    pub async fn initialize(&mut self, params: InitializeParams) -> Result<Value, anyhow::Error> {
        let root_uri = params.root_uri.map(|p| {
            if p.starts_with("file://") {
                p
            } else {
                format!("file://{p}")
            }
        });
        let workspace_folders: Option<Vec<Value>> = root_uri
            .as_ref()
            .map(|uri| vec![serde_json::json!({ "uri": uri })]);

        let init_params = serde_json::json!({
            "processId": null,
            "capabilities": {},
            "rootUri": root_uri,
            "workspaceFolders": workspace_folders,
        });
        let result = self.send_request("initialize", init_params).await?;
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

        loop {
            let raw = tokio::time::timeout(Duration::from_secs(30), self.transport.receive())
                .await
                .map_err(|_| anyhow::anyhow!("timeout waiting for response to '{method}'"))??;
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

    /// Read frames until a notification with the given method arrives.
    pub async fn wait_for_notification(&mut self, method: &str) -> Result<Value, anyhow::Error> {
        loop {
            let raw = tokio::time::timeout(Duration::from_secs(30), self.transport.receive())
                .await
                .map_err(|_| anyhow::anyhow!("timeout waiting for '{method}' notification"))??;
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

    /// Get captured sent messages from the underlying MockTransport (testing only).
    #[cfg(test)]
    pub fn mock_sent_messages(&mut self) -> Vec<Vec<u8>> {
        use crate::transport::mock::MockTransport;
        self.transport
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<MockTransport>())
            .map(|m| m.sent_messages())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::json_rpc::LspMessage;
    use crate::transport::mock::MockTransport;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_initialize_default_null_root() {
        let mock = MockTransport::new();
        mock.push_message(&LspMessage::Response {
            id: 1,
            result: Some(json!({ "capabilities": {} })),
            error: None,
        })
        .unwrap();
        let mut session = LspSession::with_transport(Box::new(mock));

        session
            .initialize(InitializeParams::default())
            .await
            .unwrap();

        let sent = session.mock_sent_messages();
        assert_eq!(sent.len(), 2);
        let init_msg = LspMessage::from_frame_bytes(&sent[0]).unwrap();
        if let LspMessage::Request { params, .. } = init_msg {
            assert_eq!(params["rootUri"], serde_json::Value::Null);
        } else {
            panic!("Expected request, got {init_msg:?}");
        }
    }

    #[tokio::test]
    async fn test_initialize_with_root_uri() {
        let mock = MockTransport::new();
        mock.push_message(&LspMessage::Response {
            id: 1,
            result: Some(json!({ "capabilities": {} })),
            error: None,
        })
        .unwrap();
        let mut session = LspSession::with_transport(Box::new(mock));

        session
            .initialize(InitializeParams {
                root_uri: Some("/home/user/project".into()),
            })
            .await
            .unwrap();

        let sent = session.mock_sent_messages();
        let init_msg = LspMessage::from_frame_bytes(&sent[0]).unwrap();
        if let LspMessage::Request { params, .. } = init_msg {
            assert_eq!(params["rootUri"], "file:///home/user/project");
        } else {
            panic!("Expected request, got {init_msg:?}");
        }
    }

    #[tokio::test]
    async fn test_initialize_root_uri_already_prefixed() {
        let mock = MockTransport::new();
        mock.push_message(&LspMessage::Response {
            id: 1,
            result: Some(json!({ "capabilities": {} })),
            error: None,
        })
        .unwrap();
        let mut session = LspSession::with_transport(Box::new(mock));

        session
            .initialize(InitializeParams {
                root_uri: Some("file:///home/user/project".into()),
            })
            .await
            .unwrap();

        let sent = session.mock_sent_messages();
        let init_msg = LspMessage::from_frame_bytes(&sent[0]).unwrap();
        if let LspMessage::Request { params, .. } = init_msg {
            assert_eq!(params["rootUri"], "file:///home/user/project");
        } else {
            panic!("Expected request, got {init_msg:?}");
        }
    }
}
