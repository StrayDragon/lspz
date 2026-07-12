//! JSON-RPC protocol types for daemon client-server communication.
//!
//! Each request has a unique `id` (u64) for matching request/response pairs.
//! The wire format is newline-delimited JSON.

use serde::{Deserialize, Serialize};

/// A request from a client to the daemon.
#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonRequest {
    /// Unique request ID for correlation.
    pub id: u64,
    /// Method name.
    pub method: String,
    /// JSON params (varies by method).
    #[serde(default = "serde_json::Value::default")]
    pub params: serde_json::Value,
}

/// A response from the daemon to a client.
#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonResponse {
    /// Matches the request ID.
    pub id: u64,
    /// `null` on success, error string on failure.
    #[serde(default)]
    pub error: Option<String>,
    /// Result value (null if error).
    #[serde(default = "serde_json::Value::default")]
    pub result: serde_json::Value,
}

impl DaemonResponse {
    /// Create a success response.
    pub fn ok(id: u64, result: serde_json::Value) -> Self {
        Self {
            id,
            error: None,
            result,
        }
    }

    /// Create an error response.
    pub fn err(id: u64, message: impl Into<String>) -> Self {
        Self {
            id,
            error: Some(message.into()),
            result: serde_json::Value::Null,
        }
    }
}

/// Parameters for `lsp/spawn` — get or create an LSP session.
#[derive(Debug, Serialize, Deserialize)]
pub struct SpawnParams {
    /// Language identifier (e.g. "rust", "go", "python").
    pub language: String,
    /// LSP server backend command (e.g. "rust-analyzer", "gopls").
    pub backend: String,
    /// Workspace root path (canonicalized absolute, no `file://` prefix).
    #[serde(default)]
    pub root_path: Option<String>,
    /// Extra arguments passed to the backend.
    #[serde(default)]
    pub extra_args: Vec<String>,
}

/// Parameters for `lsp/request` — send an LSP request and await the response.
#[derive(Debug, Serialize, Deserialize)]
pub struct LspRequestParams {
    /// Session key (language:backend:root_path).
    pub session_key: String,
    /// LSP method name.
    pub method: String,
    /// JSON params for the LSP method.
    #[serde(default = "serde_json::Value::default")]
    pub params: serde_json::Value,
}

/// Parameters for `lsp/notify` — send an LSP notification.
#[derive(Debug, Serialize, Deserialize)]
pub struct LspNotifyParams {
    /// Session key.
    pub session_key: String,
    /// LSP notification method.
    pub method: String,
    /// JSON params.
    #[serde(default = "serde_json::Value::default")]
    pub params: serde_json::Value,
}

/// Parameters for `lsp/wait_notify` — wait for a notification from the LSP server.
#[derive(Debug, Serialize, Deserialize)]
pub struct WaitNotifyParams {
    /// Session key.
    pub session_key: String,
    /// LSP notification method to wait for.
    pub method: String,
    /// Optional URI filter for diagnostics.
    #[serde(default)]
    pub filter_uri: Option<String>,
    /// Maximum time to wait, in milliseconds.
    ///
    /// The daemon enforces this deadline and always writes back a response
    /// (success or timeout error) before it elapses. This lets the client
    /// `await` the result directly instead of racing its own (longer) read
    /// timeout and then *cancelling* the future — cancellation would leave the
    /// already-written request line without a matching read, and the daemon
    /// would still complete it and write an "orphan" response line that
    /// desyncs the newline-delimited protocol.
    ///
    /// `None` falls back to the daemon/LSP session default wait.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Parameters for `lsp/sync_document` — open or update a text document.
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncDocumentParams {
    /// Session key.
    pub session_key: String,
    /// Document URI (`file://...`).
    pub uri: String,
    /// LSP language id (e.g. `rust`).
    pub language_id: String,
    /// Full document text.
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_daemon_response_ok() {
        let resp = DaemonResponse::ok(42, json!({"session_key": "abc"}));
        assert_eq!(resp.id, 42);
        assert_eq!(resp.error, None);
        assert_eq!(resp.result["session_key"], "abc");
    }

    #[test]
    fn test_daemon_response_err() {
        let resp = DaemonResponse::err(1, "something went wrong");
        assert_eq!(resp.id, 1);
        assert_eq!(resp.error.as_deref(), Some("something went wrong"));
        assert_eq!(resp.result, serde_json::Value::Null);
    }

    #[test]
    fn test_request_serialization() {
        let req = DaemonRequest {
            id: 1,
            method: "lsp/spawn".into(),
            params: json!({
                "language": "rust",
                "backend": "rust-analyzer",
            }),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: DaemonRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.method, "lsp/spawn");
    }

    #[test]
    fn test_spawn_params_repr() {
        let p = SpawnParams {
            language: "go".into(),
            backend: "gopls".into(),
            root_path: Some("/home/user/project".into()),
            extra_args: vec!["-v".into()],
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: SpawnParams = serde_json::from_str(&json).unwrap();
        assert_eq!(back.language, "go");
        assert_eq!(back.extra_args, vec!["-v"]);
    }
}
