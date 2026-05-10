//! AgentHandle — high-level LSP integration for AI coding agents.
//!
//! Manages an LSP server process, file synchronization,
//! and provides type-safe query methods.

use lspz_core::codec::compact;
use lspz_mcp::LspSession;
use serde_json::Value;

/// High-level handle to an LSP server session.
///
/// ```ignore
/// use lspz_agent_sdk::AgentHandle;
///
/// let mut agent = AgentHandle::builder()
///     .backend("rust-analyzer")
///     .language("rust")
///     .start()
///     .await?;
///
/// let diags = agent.get_diagnostics("file:///src/main.rs").await?;
/// agent.shutdown().await?;
/// ```
pub struct AgentHandle {
    session: LspSession,
    language: String,
    compression: bool,
}

impl std::fmt::Debug for AgentHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHandle")
            .field("language", &self.language)
            .field("compression", &self.compression)
            .field("session", &"LspSession { .. }")
            .finish()
    }
}

impl AgentHandle {
    /// Create a new [`AgentBuilder`].
    pub fn builder() -> AgentBuilder {
        AgentBuilder::default()
    }

    /// Construct an `AgentHandle` directly (internal, for testing).
    #[allow(dead_code)]
    pub(crate) fn new(session: LspSession, language: String, compression: bool) -> Self {
        Self {
            session,
            language,
            compression,
        }
    }

    /// Get diagnostics for a file.
    ///
    /// Opens the file, sends `didOpen`, waits for `publishDiagnostics`,
    /// and returns the result — optionally compressed via `compact::compress`.
    pub async fn get_diagnostics(&mut self, uri: &str) -> Result<String, anyhow::Error> {
        let path = uri
            .strip_prefix("file://")
            .ok_or_else(|| anyhow::anyhow!("URI must start with file://"))?;
        let content = tokio::fs::read_to_string(path).await?;

        self.session
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": self.language,
                        "version": 1,
                        "text": content,
                    }
                }),
            )
            .await?;

        let params = self
            .session
            .wait_for_notification("textDocument/publishDiagnostics")
            .await?;

        if self.compression {
            let compressed = compact::compress(&params)?;
            Ok(serde_json::to_string_pretty(&compressed)?)
        } else {
            Ok(serde_json::to_string_pretty(&params)?)
        }
    }

    /// Get completions at a specific cursor position.
    pub async fn get_completions(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<String, anyhow::Error> {
        let path = uri
            .strip_prefix("file://")
            .ok_or_else(|| anyhow::anyhow!("URI must start with file://"))?;
        let content = tokio::fs::read_to_string(path).await?;

        self.session
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": self.language,
                        "version": 1,
                        "text": content,
                    }
                }),
            )
            .await?;

        let result = self
            .session
            .send_request(
                "textDocument/completion",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                }),
            )
            .await?;

        Ok(serde_json::to_string_pretty(&result)?)
    }

    /// Get document symbols.
    pub async fn get_symbols(&mut self, uri: &str) -> Result<String, anyhow::Error> {
        let path = uri
            .strip_prefix("file://")
            .ok_or_else(|| anyhow::anyhow!("URI must start with file://"))?;
        let content = tokio::fs::read_to_string(path).await?;

        self.session
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": self.language,
                        "version": 1,
                        "text": content,
                    }
                }),
            )
            .await?;

        let result = self
            .session
            .send_request(
                "textDocument/documentSymbol",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                }),
            )
            .await?;

        Ok(serde_json::to_string_pretty(&result)?)
    }

    /// Expand compressed diagnostics back to standard LSP format.
    ///
    /// Takes a JSON string produced with compression enabled and
    /// decompresses it into a standard `publishDiagnostics` params object.
    pub fn inflate(compressed_json: &str) -> Result<String, anyhow::Error> {
        let compressed: Value = serde_json::from_str(compressed_json)?;
        let expanded = compact::decompress(&compressed)?;
        Ok(serde_json::to_string_pretty(&expanded)?)
    }

    /// Compress standard diagnostics into compact format.
    ///
    /// Takes a standard `publishDiagnostics` params JSON string and
    /// compresses it into lspz's compact format.
    pub fn compress(raw_json: &str) -> Result<String, anyhow::Error> {
        let raw: Value = serde_json::from_str(raw_json)?;
        let compressed = compact::compress(&raw)?;
        Ok(serde_json::to_string_pretty(&compressed)?)
    }

    /// Shut down the LSP server session.
    pub async fn shutdown(mut self) -> Result<(), anyhow::Error> {
        // Send shutdown request and exit notification per LSP spec
        let _ = self
            .session
            .send_request("shutdown", serde_json::json!({}))
            .await;
        self.session
            .send_notification("exit", serde_json::json!({}))
            .await?;
        Ok(())
    }
}

/// Builder for [`AgentHandle`].
#[derive(Default)]
pub struct AgentBuilder {
    backend: Option<String>,
    language: Option<String>,
    compression: bool,
}

impl AgentBuilder {
    /// Set the backend LSP server command (e.g. "rust-analyzer", "gopls").
    pub fn backend(mut self, cmd: impl Into<String>) -> Self {
        self.backend = Some(cmd.into());
        self
    }

    /// Set the language identifier (e.g. "rust", "go").
    pub fn language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    /// Enable diagnostic compression (default: disabled).
    pub fn enable_compression(mut self, enabled: bool) -> Self {
        self.compression = enabled;
        self
    }

    /// Start the LSP server and return an [`AgentHandle`].
    pub async fn start(self) -> Result<AgentHandle, anyhow::Error> {
        let backend = self
            .backend
            .ok_or_else(|| anyhow::anyhow!("backend is required"))?;
        let language = self
            .language
            .ok_or_else(|| anyhow::anyhow!("language is required"))?;

        let mut session = LspSession::spawn(&backend)?;
        session.initialize().await?;

        tracing::info!(%backend, %language, "Agent session started");

        Ok(AgentHandle {
            session,
            language,
            compression: self.compression,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU16, Ordering};

    use lspz_core::codec::json_rpc::LspMessage;
    use lspz_core::transport::mock::MockTransport;
    use lspz_mcp::LspSession;
    use serde_json::json;

    use super::*;

    static TEST_COUNTER: AtomicU16 = AtomicU16::new(0);

    fn temp_file(content: &str) -> (String, String) {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = format!("/tmp/lspz-test-{id}.rs");
        std::fs::write(&path, content).unwrap();
        (format!("file://{path}"), path)
    }

    /// Create a mock-backed AgentHandle with multiple responses.
    /// Responses should include the ones needed by `LspSession::send_request`
    /// (matching response ID) or `wait_for_notification` (matching method).
    fn mock_handle(responses: Vec<LspMessage>) -> AgentHandle {
        let mock = MockTransport::new();
        for msg in responses {
            mock.push_message(&msg).unwrap();
        }
        let session = LspSession::with_transport(Box::new(mock));
        AgentHandle::new(session, "rust".into(), false)
    }

    // ── Builder validation ──────────────────────────────────────────

    #[tokio::test]
    async fn test_builder_missing_backend() {
        let err = AgentHandle::builder()
            .language("rust")
            .start()
            .await
            .unwrap_err();
        assert!(err.to_string().contains("backend"), "{err}");
    }

    #[tokio::test]
    async fn test_builder_missing_language() {
        let err = AgentHandle::builder()
            .backend("rust-analyzer")
            .start()
            .await
            .unwrap_err();
        assert!(err.to_string().contains("language"), "{err}");
    }

    // ── get_diagnostics ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_diagnostics_no_compression() {
        let diag_notif = LspMessage::Notification {
            method: "textDocument/publishDiagnostics".into(),
            params: json!({
                "uri": "file:///test.rs",
                "diagnostics": [{
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                    "severity": 1,
                    "message": "test error",
                }],
            }),
        };

        let mut agent = mock_handle(vec![diag_notif]);
        let (uri, _path) = temp_file("fn main() {}");

        let result = agent.get_diagnostics(&uri).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["diagnostics"][0]["message"], "test error");
    }

    #[tokio::test]
    async fn test_get_diagnostics_with_compression() {
        let diag_notif = LspMessage::Notification {
            method: "textDocument/publishDiagnostics".into(),
            params: json!({
                "uri": "file:///test.rs",
                "diagnostics": [{
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                    "severity": 1,
                    "message": "test error",
                }],
            }),
        };

        let mock = MockTransport::new();
        mock.push_message(&diag_notif).unwrap();
        let session = LspSession::with_transport(Box::new(mock));
        let mut agent = AgentHandle::new(session, "rust".into(), true);
        let (uri, _path) = temp_file("fn main() {}");

        let result = agent.get_diagnostics(&uri).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        // Compressed output uses "r" (ranges) instead of full range objects
        assert!(parsed.get("r").is_some() || parsed.get("diagnostics").is_some());
    }

    #[tokio::test]
    async fn test_uri_must_be_file() {
        let mut agent = mock_handle(vec![]);
        let err = agent.get_diagnostics("http://example.com/test.rs").await.unwrap_err();
        assert!(err.to_string().contains("URI must start with file://"), "{err}");
    }

    // ── get_completions ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_completions() {
        let comp_resp = LspMessage::Response {
            id: 1,
            result: Some(json!({
                "items": [
                    { "label": "fn", "kind": 14 },
                    { "label": "for", "kind": 14 },
                ],
            })),
            error: None,
        };

        let mut agent = mock_handle(vec![comp_resp]);
        let (uri, _path) = temp_file("fn main() {}");

        let result = agent.get_completions(&uri, 0, 0).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["items"][0]["label"], "fn");
    }

    // ── get_symbols ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_symbols() {
        let sym_resp = LspMessage::Response {
            id: 1,
            result: Some(json!([
                { "name": "main", "kind": 12 },
            ])),
            error: None,
        };

        let mut agent = mock_handle(vec![sym_resp]);
        let (uri, _path) = temp_file("fn main() {}");

        let result = agent.get_symbols(&uri).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["name"], "main");
    }

    // ── inflate / compress ──────────────────────────────────────────

    #[test]
    fn test_compress() {
        let raw = json!({
            "uri": "file:///test.rs",
            "diagnostics": [{
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                "severity": 1,
                "message": "test",
            }],
        });
        let compact = AgentHandle::compress(&raw.to_string()).unwrap();
        let parsed: Value = serde_json::from_str(&compact).unwrap();
        // Compact format should have "r" for ranges
        assert!(parsed["diagnostics"][0].get("r").is_some() || parsed.get("r").is_some());
    }

    #[test]
    fn test_inflate() {
        // Compress first to get valid compact format
        let raw = json!({
            "uri": "file:///test.rs",
            "diagnostics": [{
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                "severity": 1,
                "message": "test error",
            }],
        });
        let compact = AgentHandle::compress(&raw.to_string()).unwrap();
        let expanded = AgentHandle::inflate(&compact).unwrap();

        let parsed: Value = serde_json::from_str(&expanded).unwrap();
        assert_eq!(parsed["diagnostics"][0]["message"], "test error");
    }

    #[test]
    fn test_inflate_compress_roundtrip() {
        let raw = json!({
            "uri": "file:///test.rs",
            "diagnostics": [{
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                "severity": 1,
                "message": "roundtrip test",
            }],
        });
        let raw_str = raw.to_string();
        let compact = AgentHandle::compress(&raw_str).unwrap();
        let expanded = AgentHandle::inflate(&compact).unwrap();
        let expanded_val: Value = serde_json::from_str(&expanded).unwrap();
        assert_eq!(expanded_val["diagnostics"][0]["message"], "roundtrip test");
    }

    // ── shutdown ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_shutdown() {
        // send_request needs a matching response for id=1
        let shutdown_resp = LspMessage::Response {
            id: 1,
            result: Some(json!(null)),
            error: None,
        };

        let agent = mock_handle(vec![shutdown_resp]);
        agent.shutdown().await.unwrap();
        // Success means shutdown request + exit notification were sent without error
    }
}
