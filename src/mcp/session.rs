//! LSP session — manages a single LSP server connection.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::StdioTransport;
use crate::Transport;
use crate::codec::json_rpc::LspMessage;
use serde_json::Value;

/// Cap buffered notifications to avoid unbounded growth under noisy servers.
const MAX_PENDING_NOTIFICATIONS: usize = 64;

/// Parameters for [`LspSession::initialize`].
#[derive(Default)]
pub struct InitializeParams {
    /// Workspace root URI. Auto-prefixed with `file://` if not already present.
    pub root_uri: Option<String>,
}

/// A connected LSP server session.
pub struct LspSession {
    transport: Box<dyn Transport>,
    next_id: i64,
    /// Tracks which document URIs are currently open and their latest version.
    open_documents: HashMap<String, i32>,
    /// Last time this session performed I/O. Used by the pool's idle reaper
    /// to reclaim sessions (and the underlying LSP process) that have gone
    /// quiet. Monotonic clock — process-local, never serialized.
    last_used_at: Instant,
    /// Notifications received while waiting for a request response.
    /// Consumed by [`Self::wait_for_notification_where`].
    pending_notifications: VecDeque<(String, Value)>,
}

impl LspSession {
    /// Spawn an LSP server and return an uninitialized session.
    pub fn spawn(cmd: &str) -> Result<Self, anyhow::Error> {
        Self::spawn_with_args(cmd, &[])
    }

    /// Spawn an LSP server with extra CLI arguments.
    pub fn spawn_with_args(cmd: &str, extra_args: &[String]) -> Result<Self, anyhow::Error> {
        let transport = StdioTransport::spawn(cmd, extra_args)?;
        Ok(Self {
            transport: Box::new(transport),
            next_id: 1,
            open_documents: HashMap::new(),
            last_used_at: Instant::now(),
            pending_notifications: VecDeque::new(),
        })
    }

    /// Create a session with a pre-constructed transport.
    pub fn with_transport(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            next_id: 1,
            open_documents: HashMap::new(),
            last_used_at: Instant::now(),
            pending_notifications: VecDeque::new(),
        }
    }

    /// Record that this session was just used.
    ///
    /// Every public I/O method calls this on entry so the pool's idle reaper
    /// sees fresh activity and does not reclaim a session mid-conversation.
    fn touch(&mut self) {
        self.last_used_at = Instant::now();
    }

    /// Buffer a server notification for later [`wait_for_notification_where`].
    fn buffer_notification(&mut self, method: String, params: Value) {
        if self.pending_notifications.len() >= MAX_PENDING_NOTIFICATIONS {
            let dropped = self.pending_notifications.pop_front();
            if let Some((m, _)) = dropped {
                tracing::warn!(
                    method = %m,
                    cap = MAX_PENDING_NOTIFICATIONS,
                    "Pending notification buffer full; dropping oldest"
                );
            }
        }
        tracing::trace!(method = %method, "Buffered notification");
        self.pending_notifications.push_back((method, params));
    }

    /// Last time this session performed I/O (monotonic, process-local).
    ///
    /// The daemon's idle reaper uses this to drop sessions whose language
    /// server has been quiet for too long.
    pub fn last_used_at(&self) -> Instant {
        self.last_used_at
    }

    /// Perform the LSP initialize/initialized handshake.
    pub async fn initialize(&mut self, params: InitializeParams) -> Result<Value, anyhow::Error> {
        self.touch();
        let root_uri = params.root_uri.map(|p| {
            if p.starts_with("file://") {
                p
            } else {
                format!("file://{p}")
            }
        });
        let workspace_folders: Option<Vec<Value>> = root_uri.as_ref().map(|uri| {
            let name = uri
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or("workspace");
            vec![serde_json::json!({ "uri": uri, "name": name })]
        });

        let init_params = serde_json::json!({
            "processId": null,
            "capabilities": {},
            "rootUri": root_uri,
            "workspaceFolders": workspace_folders,
            "clientInfo": {
                "name": "lspz",
                "version": env!("CARGO_PKG_VERSION"),
            },
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
        self.touch();
        let id = self.next_id;
        self.next_id += 1;
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
            self.touch();
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
                LspMessage::Notification { method: m, params } => {
                    self.buffer_notification(m, params);
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
        self.touch();
        let msg = LspMessage::Notification {
            method: method.into(),
            params,
        };
        let frame = msg.to_bytes()?;
        self.transport.send(&frame).await?;
        Ok(())
    }

    /// Ensure a document is open in the LSP server, sending `didOpen` on the
    /// first call and `didChange` (full document sync) on subsequent calls.
    ///
    /// This avoids re-sending `didOpen` for already-open documents, which can
    /// cause the LSP server to reset its internal state and return stale
    /// diagnostics.
    pub async fn open_or_update_document(
        &mut self,
        uri: &str,
        language_id: &str,
        content: &str,
    ) -> Result<(), anyhow::Error> {
        self.touch();
        let new_version = if let Some(version) = self.open_documents.get_mut(uri) {
            *version += 1;
            Some(*version)
        } else {
            None
        };

        if let Some(version) = new_version {
            // Document already open — send didChange with incremented version.
            self.send_notification(
                "textDocument/didChange",
                serde_json::json!({
                    "textDocument": { "uri": uri, "version": version },
                    "contentChanges": [{ "text": content }],
                }),
            )
            .await?;
        } else {
            // First time — send didOpen.
            self.send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": language_id,
                        "version": 1,
                        "text": content,
                    }
                }),
            )
            .await?;
            self.open_documents.insert(uri.to_string(), 1);
        }
        Ok(())
    }

    /// Mark a document closed in the session tracker after sending `didClose`.
    pub fn close_document(&mut self, uri: &str) {
        self.open_documents.remove(uri);
    }

    /// Returns `true` if the URI is currently tracked as open.
    pub fn is_document_open(&self, uri: &str) -> bool {
        self.open_documents.contains_key(uri)
    }

    /// Read frames until a notification with the given method arrives.
    pub async fn wait_for_notification(&mut self, method: &str) -> Result<Value, anyhow::Error> {
        self.wait_for_notification_where(method, |_| true).await
    }

    /// Read frames until a notification with the given method arrives and the
    /// predicate returns `true` for its params.
    pub async fn wait_for_notification_where(
        &mut self,
        method: &str,
        predicate: impl Fn(&Value) -> bool,
    ) -> Result<Value, anyhow::Error> {
        self.touch();

        // Prefer notifications buffered during earlier send_request waits.
        if let Some(idx) = self
            .pending_notifications
            .iter()
            .position(|(m, p)| m == method && predicate(p))
        {
            let (_, params) = self
                .pending_notifications
                .remove(idx)
                .expect("index from position");
            return Ok(params);
        }

        loop {
            let raw = tokio::time::timeout(Duration::from_secs(30), self.transport.receive())
                .await
                .map_err(|_| anyhow::anyhow!("timeout waiting for '{method}' notification"))??;
            self.touch();
            let parsed = LspMessage::from_frame_bytes(&raw)?;
            match parsed {
                LspMessage::Notification { method: m, params }
                    if m == method && predicate(&params) =>
                {
                    return Ok(params);
                }
                LspMessage::Notification { method: m, params } => {
                    self.buffer_notification(m, params);
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

    #[tokio::test]
    async fn test_open_or_update_first_call_sends_did_open() {
        let mock = MockTransport::new();
        let mut session = LspSession::with_transport(Box::new(mock));

        session
            .open_or_update_document("file:///test.py", "python", "print('hi')")
            .await
            .unwrap();

        let sent = session.mock_sent_messages();
        assert_eq!(sent.len(), 1);
        let msg = LspMessage::from_frame_bytes(&sent[0]).unwrap();
        if let LspMessage::Notification { method, params } = msg {
            assert_eq!(method, "textDocument/didOpen");
            assert_eq!(params["textDocument"]["uri"], "file:///test.py");
            assert_eq!(params["textDocument"]["version"], 1);
            assert_eq!(params["textDocument"]["languageId"], "python");
        } else {
            panic!("Expected notification, got {msg:?}");
        }
    }

    #[tokio::test]
    async fn test_open_or_update_second_call_sends_did_change() {
        let mock = MockTransport::new();
        let mut session = LspSession::with_transport(Box::new(mock));

        // First call — didOpen
        session
            .open_or_update_document("file:///test.py", "python", "v1")
            .await
            .unwrap();

        // Second call — didChange with version 2
        session
            .open_or_update_document("file:///test.py", "python", "v2")
            .await
            .unwrap();

        let sent = session.mock_sent_messages();
        assert_eq!(sent.len(), 2);

        let msg1 = LspMessage::from_frame_bytes(&sent[0]).unwrap();
        if let LspMessage::Notification { method, .. } = msg1 {
            assert_eq!(method, "textDocument/didOpen");
        } else {
            panic!("Expected notification, got {msg1:?}");
        }

        let msg2 = LspMessage::from_frame_bytes(&sent[1]).unwrap();
        if let LspMessage::Notification { method, params } = msg2 {
            assert_eq!(method, "textDocument/didChange");
            assert_eq!(params["textDocument"]["version"], 2);
            assert_eq!(params["contentChanges"][0]["text"], "v2");
        } else {
            panic!("Expected notification, got {msg2:?}");
        }
    }

    #[tokio::test]
    async fn test_open_or_update_increments_version() {
        let mock = MockTransport::new();
        let mut session = LspSession::with_transport(Box::new(mock));

        session
            .open_or_update_document("file:///a.rs", "rust", "1")
            .await
            .unwrap();
        session
            .open_or_update_document("file:///a.rs", "rust", "2")
            .await
            .unwrap();
        session
            .open_or_update_document("file:///a.rs", "rust", "3")
            .await
            .unwrap();

        let sent = session.mock_sent_messages();
        assert_eq!(sent.len(), 3);

        // First is didOpen (version 1)
        let msg0 = LspMessage::from_frame_bytes(&sent[0]).unwrap();
        if let LspMessage::Notification { method, params } = msg0 {
            assert_eq!(method, "textDocument/didOpen");
            assert_eq!(params["textDocument"]["version"], 1);
        }

        // Second is didChange (version 2)
        let msg1 = LspMessage::from_frame_bytes(&sent[1]).unwrap();
        if let LspMessage::Notification { method, params } = msg1 {
            assert_eq!(method, "textDocument/didChange");
            assert_eq!(params["textDocument"]["version"], 2);
        }

        // Third is didChange (version 3)
        let msg2 = LspMessage::from_frame_bytes(&sent[2]).unwrap();
        if let LspMessage::Notification { method, params } = msg2 {
            assert_eq!(method, "textDocument/didChange");
            assert_eq!(params["textDocument"]["version"], 3);
        }
    }

    #[tokio::test]
    async fn test_open_or_update_independent_documents() {
        let mock = MockTransport::new();
        let mut session = LspSession::with_transport(Box::new(mock));

        session
            .open_or_update_document("file:///a.py", "python", "a")
            .await
            .unwrap();
        session
            .open_or_update_document("file:///b.py", "python", "b")
            .await
            .unwrap();
        session
            .open_or_update_document("file:///a.py", "python", "a2")
            .await
            .unwrap();

        let sent = session.mock_sent_messages();
        assert_eq!(sent.len(), 3);

        // a.py — didOpen
        let msg0 = LspMessage::from_frame_bytes(&sent[0]).unwrap();
        if let LspMessage::Notification { method, params } = msg0 {
            assert_eq!(method, "textDocument/didOpen");
            assert_eq!(params["textDocument"]["uri"], "file:///a.py");
        }

        // b.py — didOpen (independent)
        let msg1 = LspMessage::from_frame_bytes(&sent[1]).unwrap();
        if let LspMessage::Notification { method, params } = msg1 {
            assert_eq!(method, "textDocument/didOpen");
            assert_eq!(params["textDocument"]["uri"], "file:///b.py");
        }

        // a.py — didChange (version 2, b.py didn't affect it)
        let msg2 = LspMessage::from_frame_bytes(&sent[2]).unwrap();
        if let LspMessage::Notification { method, params } = msg2 {
            assert_eq!(method, "textDocument/didChange");
            assert_eq!(params["textDocument"]["uri"], "file:///a.py");
            assert_eq!(params["textDocument"]["version"], 2);
        }
    }

    #[tokio::test]
    async fn test_send_request_buffers_interleaved_notifications() {
        let mock = MockTransport::new();
        // While waiting for hover response id=1, a diagnostics notify arrives first.
        mock.push_message(&LspMessage::Notification {
            method: "textDocument/publishDiagnostics".into(),
            params: json!({
                "uri": "file:///t.rs",
                "diagnostics": []
            }),
        })
        .unwrap();
        mock.push_message(&LspMessage::Response {
            id: 1,
            result: Some(json!({"contents": "doc"})),
            error: None,
        })
        .unwrap();

        let mut session = LspSession::with_transport(Box::new(mock));
        let result = session
            .send_request("textDocument/hover", json!({}))
            .await
            .unwrap();
        assert_eq!(result["contents"], "doc");

        let diags = session
            .wait_for_notification("textDocument/publishDiagnostics")
            .await
            .unwrap();
        assert_eq!(diags["uri"], "file:///t.rs");
    }
}
