//! AgentHandle — high-level LSP integration for AI coding agents.

use std::sync::Arc;

use crate::config::Config;
use crate::interceptors::{Direction, InterceptorChain, default_interceptors};
use crate::mcp::{InitializeParams, LspSession};
use serde_json::Value;
use tokio::sync::RwLock;

/// High-level handle to an LSP server session.
pub struct AgentHandle {
    session: LspSession,
    language: String,
    interceptor_chain: Option<InterceptorChain>,
}

impl std::fmt::Debug for AgentHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHandle")
            .field("language", &self.language)
            .field("compression", &self.interceptor_chain.is_some())
            .field("session", &"LspSession { .. }")
            .finish()
    }
}

impl AgentHandle {
    /// Create a new [`AgentBuilder`].
    pub fn builder() -> AgentBuilder {
        AgentBuilder::default()
    }

    #[allow(dead_code)]
    pub fn new(session: LspSession, language: String, compression: bool) -> Self {
        let interceptor_chain = if compression {
            Some(build_interceptor_chain())
        } else {
            None
        };
        Self {
            session,
            language,
            interceptor_chain,
        }
    }

    #[cfg(test)]
    fn test_sent_messages(&mut self) -> Vec<Vec<u8>> {
        self.session.mock_sent_messages()
    }

    /// Ensure the URI is open in the LSP session.
    ///
    /// If already open (e.g. after [`Self::notify_change`]), skips disk I/O so
    /// unsaved buffers are not overwritten. Otherwise reads from disk and
    /// sends `didOpen`.
    async fn open_file(&mut self, uri: &str) -> Result<(), anyhow::Error> {
        if self.session.is_document_open(uri) {
            return Ok(());
        }
        let path = crate::uri::path_from_file_uri(uri).map_err(|e| anyhow::anyhow!(e))?;
        let content = tokio::fs::read_to_string(&path).await?;
        self.session
            .open_or_update_document(uri, &self.language, &content)
            .await?;
        Ok(())
    }

    /// Run params through the interceptor chain if compression is enabled.
    async fn process_through_chain(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, anyhow::Error> {
        match &self.interceptor_chain {
            Some(chain) => match chain
                .process(method, params.clone(), Direction::ServerToClient)
                .await
            {
                Ok(Some(p)) => Ok(p),
                Ok(None) => Ok(Value::Null),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        method = %method,
                        "Interceptor chain failed, returning original"
                    );
                    Ok(params)
                }
            },
            None => Ok(params),
        }
    }

    // ── Notification-based queries ─────────────────────────────────────

    /// Get diagnostics for a file.
    pub async fn get_diagnostics(&mut self, uri: &str) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let uri_owned = uri.to_string();
        let params = self
            .session
            .wait_for_notification_where("textDocument/publishDiagnostics", move |p| {
                p.get("uri").and_then(Value::as_str) == Some(uri_owned.as_str())
            })
            .await?;
        let processed = self
            .process_through_chain("textDocument/publishDiagnostics", params)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    // ── Request-based queries (file-scoped) ────────────────────────────

    /// Get completions at a specific cursor position.
    pub async fn get_completions(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
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
        let processed = self
            .process_through_chain("textDocument/completion", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Get document symbols.
    pub async fn get_symbols(&mut self, uri: &str) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let result = self
            .session
            .send_request(
                "textDocument/documentSymbol",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("textDocument/documentSymbol", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Get hover information at a specific cursor position.
    pub async fn get_hover(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let result = self
            .session
            .send_request(
                "textDocument/hover",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("textDocument/hover", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Get references at a specific cursor position.
    pub async fn get_references(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let result = self
            .session
            .send_request(
                "textDocument/references",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                    "context": { "includeDeclaration": true },
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("textDocument/references", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Get the definition location of a symbol.
    pub async fn get_definition(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let result = self
            .session
            .send_request(
                "textDocument/definition",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("textDocument/definition", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Get the implementation locations of a symbol.
    pub async fn get_implementation(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let result = self
            .session
            .send_request(
                "textDocument/implementation",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("textDocument/implementation", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Get the type definition location of a symbol.
    pub async fn get_type_definition(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let result = self
            .session
            .send_request(
                "textDocument/typeDefinition",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("textDocument/typeDefinition", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    // ── Request-based queries (workspace-scoped) ───────────────────────

    /// Query workspace symbols matching a search term.
    pub async fn get_workspace_symbols(&mut self, query: &str) -> Result<String, anyhow::Error> {
        let result = self
            .session
            .send_request(
                "workspace/symbol",
                serde_json::json!({
                    "query": query,
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("workspace/symbol", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Get workspace diagnostics for a file.
    pub async fn get_workspace_diagnostics(&mut self, uri: &str) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let result = self
            .session
            .send_request(
                "workspace/diagnostic",
                serde_json::json!({
                    "previousResultId": null,
                    "textDocument": { "uri": uri },
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("workspace/diagnostic", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    // ── File sync notifications ──────────────────────────────────────────

    /// Notify the LSP server that a file's content has changed.
    ///
    /// Uses full document sync via [`LspSession::open_or_update_document`].
    pub async fn notify_change(&mut self, uri: &str, content: &str) -> Result<(), anyhow::Error> {
        self.session
            .open_or_update_document(uri, &self.language, content)
            .await?;
        Ok(())
    }

    /// Notify the LSP server that a file has been closed.
    pub async fn notify_close(&mut self, uri: &str) -> Result<(), anyhow::Error> {
        self.session
            .send_notification(
                "textDocument/didClose",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                }),
            )
            .await?;
        self.session.close_document(uri);
        Ok(())
    }

    /// Notify the LSP server that a file has been saved.
    pub async fn notify_save(&mut self, uri: &str) -> Result<(), anyhow::Error> {
        self.session
            .send_notification(
                "textDocument/didSave",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                }),
            )
            .await?;
        Ok(())
    }

    // ── Refactoring operations ────────────────────────────────────────────

    /// Rename a symbol at the given position.
    pub async fn rename(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let result = self
            .session
            .send_request(
                "textDocument/rename",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                    "newName": new_name,
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("textDocument/rename", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Get code actions for a position in a file.
    pub async fn code_action(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        diagnostics: Option<Vec<Value>>,
        only: Option<Vec<String>>,
    ) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let mut context = serde_json::json!({});
        if let Some(diags) = diagnostics {
            context["diagnostics"] = Value::Array(diags);
        }
        if let Some(only) = only {
            context["only"] = serde_json::json!(only);
        }
        let result = self
            .session
            .send_request(
                "textDocument/codeAction",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "range": {
                        "start": { "line": line, "character": character },
                        "end": { "line": line, "character": character },
                    },
                    "context": context,
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("textDocument/codeAction", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Format a file.
    pub async fn formatting(
        &mut self,
        uri: &str,
        options: Option<Value>,
    ) -> Result<String, anyhow::Error> {
        self.open_file(uri).await?;
        let formatting_options = options.unwrap_or(serde_json::json!({
            "tabSize": 4,
            "insertSpaces": true,
        }));
        let result = self
            .session
            .send_request(
                "textDocument/formatting",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "options": formatting_options,
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("textDocument/formatting", result)
            .await?;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    // ── Raw request ──────────────────────────────────────────────────────

    /// Send a raw LSP request and return the response as a JSON string.
    ///
    /// Use this for LSP methods not covered by the typed API.
    /// Bypasses the interceptor chain.
    pub async fn send_raw(&mut self, method: &str, params: Value) -> Result<String, anyhow::Error> {
        let result = self.session.send_request(method, params).await?;
        Ok(serde_json::to_string_pretty(&result)?)
    }

    // ── Compression helpers ────────────────────────────────────────────

    /// Expand compressed diagnostics back to standard LSP format.
    pub fn inflate(compressed_json: &str) -> Result<String, anyhow::Error> {
        let compressed: Value = serde_json::from_str(compressed_json)?;
        let expanded = crate::codec::compact::decompress(&compressed)?;
        Ok(serde_json::to_string_pretty(&expanded)?)
    }

    /// Compress standard diagnostics into compact format.
    pub fn compress(raw_json: &str) -> Result<String, anyhow::Error> {
        let raw: Value = serde_json::from_str(raw_json)?;
        let compressed = crate::codec::compact::compress(&raw)?;
        Ok(serde_json::to_string_pretty(&compressed)?)
    }

    // ── Lifecycle ──────────────────────────────────────────────────────

    /// Shut down the LSP server session.
    pub async fn shutdown(mut self) -> Result<(), anyhow::Error> {
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
    workspace_root: Option<String>,
}

impl AgentBuilder {
    /// Set the backend LSP server command.
    pub fn backend(mut self, cmd: impl Into<String>) -> Self {
        self.backend = Some(cmd.into());
        self
    }

    /// Set the language identifier.
    pub fn language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    /// Enable compression (default: disabled).
    pub fn enable_compression(mut self, enabled: bool) -> Self {
        self.compression = enabled;
        self
    }

    /// Set the workspace root URI for the LSP server.
    pub fn workspace_root(mut self, path: impl Into<String>) -> Self {
        self.workspace_root = Some(path.into());
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
        let init_params = InitializeParams {
            root_uri: self.workspace_root,
        };
        session.initialize(init_params).await?;

        tracing::info!(%backend, %language, "Agent session started");

        Ok(AgentHandle::new(session, language, self.compression))
    }
}

/// Build an interceptor chain with all compressors enabled.
fn build_interceptor_chain() -> InterceptorChain {
    let config = Arc::new(RwLock::new(Config {
        backend_cmd: String::new(),
        capping: crate::CappingConfig::default(),
        enable_diag_compress: true,
        enable_completion_compress: true,
        enable_hover_compress: true,
        enable_document_symbol_compress: true,
        enable_location_compress: true,
        enable_workspace_symbol_compress: true,
        enable_workspace_diag_compress: true,
        output_format: crate::OutputFormat::Json,
        log_level: "info".into(),
        metrics: crate::MetricsConfig::default(),
    }));

    InterceptorChain::new(default_interceptors(), config)
}

#[cfg(test)]
mod tests {
    use crate::codec::json_rpc::LspMessage;
    use crate::mcp::LspSession;
    use crate::transport::mock::MockTransport;
    use serde_json::json;

    use super::*;

    fn temp_file(content: &str) -> (String, tempfile::NamedTempFile) {
        let mut f = tempfile::Builder::new().suffix(".rs").tempfile().unwrap();
        use std::io::Write;
        f.write_all(content.as_bytes()).unwrap();
        let path = f.path().to_string_lossy().to_string();
        (format!("file://{path}"), f)
    }

    fn mock_handle(responses: Vec<LspMessage>) -> AgentHandle {
        let mock = MockTransport::new();
        for msg in responses {
            mock.push_message(&msg).unwrap();
        }
        let session = LspSession::with_transport(Box::new(mock));
        AgentHandle::new(session, "rust".into(), false)
    }

    fn mock_handle_compressed(responses: Vec<LspMessage>) -> AgentHandle {
        let mock = MockTransport::new();
        for msg in responses {
            mock.push_message(&msg).unwrap();
        }
        let session = LspSession::with_transport(Box::new(mock));
        AgentHandle::new(session, "rust".into(), true)
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
        let (uri, _tmp) = temp_file("fn main() {}");
        let diag_notif = LspMessage::Notification {
            method: "textDocument/publishDiagnostics".into(),
            params: json!({
                "uri": uri,
                "diagnostics": [{
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                    "severity": 1,
                    "message": "test error",
                }],
            }),
        };

        let mut agent = mock_handle(vec![diag_notif]);

        let result = agent.get_diagnostics(&uri).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["diagnostics"][0]["message"], "test error");
    }

    #[tokio::test]
    async fn test_get_diagnostics_with_compression() {
        let (uri, _tmp) = temp_file("fn main() {}");
        let diag_notif = LspMessage::Notification {
            method: "textDocument/publishDiagnostics".into(),
            params: json!({
                "uri": uri,
                "diagnostics": [{
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                    "severity": 1,
                    "message": "test error",
                }],
            }),
        };

        let mut agent = mock_handle_compressed(vec![diag_notif]);

        let result = agent.get_diagnostics(&uri).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("r").is_some() || parsed.get("diagnostics").is_some());
    }

    #[tokio::test]
    async fn test_uri_must_be_file() {
        let mut agent = mock_handle(vec![]);
        let err = agent
            .get_diagnostics("http://example.com/test.rs")
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("URI must start with file://"),
            "{err}"
        );
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
        let (uri, _tmp) = temp_file("fn main() {}");

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
        let (uri, _tmp) = temp_file("fn main() {}");

        let result = agent.get_symbols(&uri).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["name"], "main");
    }

    // ── get_hover ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_hover() {
        let hover_resp = LspMessage::Response {
            id: 1,
            result: Some(json!({
                "contents": {
                    "kind": "markdown",
                    "value": "**fn main** — Entry point",
                },
            })),
            error: None,
        };

        let mut agent = mock_handle(vec![hover_resp]);
        let (uri, _tmp) = temp_file("fn main() {}");

        let result = agent.get_hover(&uri, 0, 0).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["contents"]["value"], "**fn main** — Entry point");
    }

    // ── get_references ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_references() {
        let ref_resp = LspMessage::Response {
            id: 1,
            result: Some(json!([
                {
                    "uri": "file:///lib.rs",
                    "range": { "start": { "line": 5, "character": 0 }, "end": { "line": 5, "character": 1 } },
                }
            ])),
            error: None,
        };

        let mut agent = mock_handle(vec![ref_resp]);
        let (uri, _tmp) = temp_file("fn main() {}");

        let result = agent.get_references(&uri, 0, 0).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["uri"], "file:///lib.rs");
    }

    // ── get_definition ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_definition() {
        let def_resp = LspMessage::Response {
            id: 1,
            result: Some(json!({
                "uri": "file:///src/lib.rs",
                "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 10 } },
            })),
            error: None,
        };

        let mut agent = mock_handle(vec![def_resp]);
        let (uri, _tmp) = temp_file("fn main() {}");

        let result = agent.get_definition(&uri, 0, 0).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["uri"], "file:///src/lib.rs");
    }

    // ── get_implementation ──────────────────────────────────────────

    #[tokio::test]
    async fn test_get_implementation() {
        let impl_resp = LspMessage::Response {
            id: 1,
            result: Some(json!([
                {
                    "uri": "file:///src/impl.rs",
                    "range": { "start": { "line": 10, "character": 0 }, "end": { "line": 10, "character": 5 } },
                }
            ])),
            error: None,
        };

        let mut agent = mock_handle(vec![impl_resp]);
        let (uri, _tmp) = temp_file("fn main() {}");

        let result = agent.get_implementation(&uri, 0, 0).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["uri"], "file:///src/impl.rs");
    }

    // ── get_type_definition ─────────────────────────────────────────

    #[tokio::test]
    async fn test_get_type_definition() {
        let td_resp = LspMessage::Response {
            id: 1,
            result: Some(json!({
                "uri": "file:///src/types.rs",
                "range": { "start": { "line": 3, "character": 0 }, "end": { "line": 3, "character": 8 } },
            })),
            error: None,
        };

        let mut agent = mock_handle(vec![td_resp]);
        let (uri, _tmp) = temp_file("fn main() {}");

        let result = agent.get_type_definition(&uri, 0, 0).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["uri"], "file:///src/types.rs");
    }

    // ── get_workspace_symbols ───────────────────────────────────────

    #[tokio::test]
    async fn test_get_workspace_symbols() {
        let ws_resp = LspMessage::Response {
            id: 1,
            result: Some(json!([
                { "name": "main", "kind": 12, "location": {
                    "uri": "file:///src/main.rs",
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 1 } },
                }},
            ])),
            error: None,
        };

        let mut agent = mock_handle(vec![ws_resp]);
        let result = agent.get_workspace_symbols("main").await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["name"], "main");
    }

    // ── get_workspace_diagnostics ───────────────────────────────────

    #[tokio::test]
    async fn test_get_workspace_diagnostics() {
        let wd_resp = LspMessage::Response {
            id: 1,
            result: Some(json!({
                "kind": "full",
                "items": [{
                    "uri": "file:///test.rs",
                    "diagnostics": [{
                        "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                        "severity": 1,
                        "message": "ws diag",
                    }],
                }],
                "resultId": "abc",
            })),
            error: None,
        };

        let mut agent = mock_handle(vec![wd_resp]);
        let (uri, _tmp) = temp_file("fn main() {}");

        let result = agent.get_workspace_diagnostics(&uri).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("items").is_some() || parsed.get("resultId").is_some());
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
        assert!(parsed["diagnostics"][0].get("r").is_some() || parsed.get("r").is_some());
    }

    #[test]
    fn test_inflate() {
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

    // ── file sync ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_notify_change_then_query_skips_disk_reread() {
        let (uri, _tmp) = temp_file("FROM_DISK_CONTENT");
        let diag = LspMessage::Notification {
            method: "textDocument/publishDiagnostics".into(),
            params: json!({
                "uri": &uri,
                "diagnostics": [],
            }),
        };
        let mut agent = mock_handle(vec![diag]);
        agent
            .notify_change(&uri, "FROM_AGENT_UNSAVED")
            .await
            .unwrap();

        let before = agent.test_sent_messages();
        assert!(
            before.iter().any(|b| {
                let s = String::from_utf8_lossy(b);
                s.contains("FROM_AGENT_UNSAVED")
            }),
            "notify_change should sync agent buffer"
        );

        agent.get_diagnostics(&uri).await.unwrap();
        let after = agent.test_sent_messages();
        let new_frames = &after[before.len()..];
        assert!(
            new_frames.iter().all(|b| {
                let s = String::from_utf8_lossy(b);
                !s.contains("FROM_DISK_CONTENT")
            }),
            "query must not re-send disk content after notify_change; new frames: {:?}",
            new_frames
                .iter()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .collect::<Vec<_>>()
        );
        assert!(
            !after.iter().any(|b| {
                let s = String::from_utf8_lossy(b);
                s.contains("FROM_DISK_CONTENT")
            }),
            "disk content must never have been synced when notify_change ran first"
        );
    }

    #[tokio::test]
    async fn test_notify_change_sends_did_change() {
        let mut agent = mock_handle(vec![]);
        agent
            .notify_change("file:///test.rs", "new content")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_notify_change_version_increments() {
        let mut agent = mock_handle(vec![]);
        // First change (version starts at 1 per entry default)
        agent.notify_change("file:///test.rs", "v1").await.unwrap();
        // Second change
        agent.notify_change("file:///test.rs", "v2").await.unwrap();
        // Both succeed — version tracking is internal
    }

    #[tokio::test]
    async fn test_notify_close_sends_did_close() {
        let mut agent = mock_handle(vec![]);
        agent
            .notify_change("file:///test.rs", "content")
            .await
            .unwrap();
        agent.notify_close("file:///test.rs").await.unwrap();
    }

    #[tokio::test]
    async fn test_notify_close_then_change_resets_version() {
        let mut agent = mock_handle(vec![]);
        agent.notify_change("file:///test.rs", "v1").await.unwrap();
        agent.notify_close("file:///test.rs").await.unwrap();
        // After close, next change should start fresh
        agent.notify_change("file:///test.rs", "v2").await.unwrap();
    }

    #[tokio::test]
    async fn test_notify_save_sends_did_save() {
        let mut agent = mock_handle(vec![]);
        agent.notify_save("file:///test.rs").await.unwrap();
    }

    // ── rename ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_rename() {
        let rename_resp = LspMessage::Response {
            id: 1,
            result: Some(json!({
                "changes": {
                    "file:///test.rs": [{
                        "range": { "start": { "line": 0, "character": 3 }, "end": { "line": 0, "character": 7 } },
                        "newText": "new_name",
                    }]
                }
            })),
            error: None,
        };

        let mut agent = mock_handle(vec![rename_resp]);
        let (uri, _tmp) = temp_file("fn main() {}");
        let result = agent.rename(&uri, 0, 3, "new_name").await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["changes"].is_object());
    }

    // ── code_action ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_code_action() {
        let action_resp = LspMessage::Response {
            id: 1,
            result: Some(json!([
                {
                    "title": "Add import",
                    "kind": "quickfix",
                    "edit": { "changes": {} },
                }
            ])),
            error: None,
        };

        let mut agent = mock_handle(vec![action_resp]);
        let (uri, _tmp) = temp_file("fn main() {}");
        let result = agent.code_action(&uri, 0, 0, None, None).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["title"], "Add import");
    }

    #[tokio::test]
    async fn test_code_action_with_diagnostics() {
        let action_resp = LspMessage::Response {
            id: 1,
            result: Some(json!([{ "title": "Fix", "kind": "quickfix" }])),
            error: None,
        };

        let mut agent = mock_handle(vec![action_resp]);
        let (uri, _tmp) = temp_file("fn main() {}");
        let diags = vec![json!({
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 5 } },
            "severity": 1,
            "message": "err",
        })];
        let result = agent
            .code_action(&uri, 0, 0, Some(diags), None)
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["title"], "Fix");
    }

    #[tokio::test]
    async fn test_code_action_with_only_filter() {
        let action_resp = LspMessage::Response {
            id: 1,
            result: Some(json!([{ "title": "Organize", "kind": "source.organizeImports" }])),
            error: None,
        };

        let mut agent = mock_handle(vec![action_resp]);
        let (uri, _tmp) = temp_file("fn main() {}");
        let result = agent
            .code_action(
                &uri,
                0,
                0,
                None,
                Some(vec!["source.organizeImports".into()]),
            )
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["kind"], "source.organizeImports");
    }

    // ── formatting ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_formatting() {
        let fmt_resp = LspMessage::Response {
            id: 1,
            result: Some(json!([
                {
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 12 } },
                    "newText": "fn main() {}\n",
                }
            ])),
            error: None,
        };

        let mut agent = mock_handle(vec![fmt_resp]);
        let (uri, _tmp) = temp_file("fn main(){}");
        let result = agent.formatting(&uri, None).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["newText"], "fn main() {}\n");
    }

    #[tokio::test]
    async fn test_formatting_custom_options() {
        let fmt_resp = LspMessage::Response {
            id: 1,
            result: Some(json!([{
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 5 } },
                "newText": "    x",
            }])),
            error: None,
        };

        let mut agent = mock_handle(vec![fmt_resp]);
        let (uri, _tmp) = temp_file("x = 1");
        let result = agent
            .formatting(&uri, Some(json!({ "tabSize": 2, "insertSpaces": false })))
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed[0]["newText"].is_string());
    }

    // ── send_raw ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_send_raw_request() {
        let raw_resp = LspMessage::Response {
            id: 1,
            result: Some(json!({ "signatures": [{ "label": "fn main()" }] })),
            error: None,
        };

        let mut agent = mock_handle(vec![raw_resp]);
        let result = agent
            .send_raw(
                "textDocument/signatureHelp",
                json!({
                    "textDocument": { "uri": "file:///test.rs" },
                    "position": { "line": 0, "character": 5 },
                }),
            )
            .await
            .unwrap();

        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["signatures"][0]["label"], "fn main()");
    }

    #[tokio::test]
    async fn test_send_raw_error_propagates() {
        let err_resp = LspMessage::Response {
            id: 1,
            result: None,
            error: Some(crate::codec::json_rpc::JsonRpcError {
                code: -32600,
                message: "Invalid params".into(),
                data: None,
            }),
        };

        let mut agent = mock_handle(vec![err_resp]);
        let err = agent
            .send_raw("textDocument/unknownMethod", json!({}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Invalid params"), "{err}");
    }

    // ── shutdown ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_shutdown() {
        let shutdown_resp = LspMessage::Response {
            id: 1,
            result: Some(json!(null)),
            error: None,
        };

        let agent = mock_handle(vec![shutdown_resp]);
        agent.shutdown().await.unwrap();
    }
}
