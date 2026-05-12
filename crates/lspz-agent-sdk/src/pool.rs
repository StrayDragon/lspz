//! AgentPool — multi-language LSP session management for AI agents.
//!
//! Manages multiple LSP server sessions keyed by language identifier,
//! with lazy initialization on first use.

use std::collections::HashMap;
use std::sync::Arc;

use lspz_core::interceptors::{Direction, Interceptor, InterceptorChain};
use lspz_mcp::LspSession;
use tokio::sync::RwLock;

use crate::AgentHandle;

/// A pool of LSP sessions managed by language identifier.
///
/// Sessions are lazily spawned on the first query for a given language.
///
/// # Example
///
/// ```ignore
/// use lspz_agent_sdk::AgentPool;
///
/// let mut pool = AgentPool::builder()
///     .register("rust", "rust-analyzer")
///     .register("go", "gopls")
///     .enable_compression(true)
///     .start_all().await?;
///
/// let diags = pool.get_diagnostics("file:///src/main.rs", "rust").await?;
/// pool.shutdown_all().await?;
/// ```
pub struct AgentPool {
    sessions: HashMap<String, LspSession>,
    backends: HashMap<String, String>,
    interceptor_chain: Option<InterceptorChain>,
}

impl std::fmt::Debug for AgentPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentPool")
            .field("languages", &self.backends.keys().collect::<Vec<_>>())
            .field("active_sessions", &self.sessions.len())
            .field("compression", &self.interceptor_chain.is_some())
            .finish()
    }
}

impl AgentPool {
    /// Create a new [`AgentPoolBuilder`].
    pub fn builder() -> AgentPoolBuilder {
        AgentPoolBuilder::default()
    }

    /// Get or lazily spawn a session for the given language.
    async fn session_for(&mut self, language: &str) -> Result<&mut LspSession, anyhow::Error> {
        if !self.sessions.contains_key(language) {
            let cmd = self.backends.get(language).ok_or_else(|| {
                anyhow::anyhow!("no backend registered for language '{language}'")
            })?;
            let mut session = LspSession::spawn(cmd)?;
            session.initialize().await?;
            tracing::info!(language, "LSP session initialized");
            self.sessions.insert(language.to_owned(), session);
        }
        Ok(self.sessions.get_mut(language).unwrap())
    }

    /// Insert a pre-configured session (for testing with mock transports).
    #[allow(dead_code)]
    pub(crate) fn insert_session(&mut self, language: &str, session: LspSession) {
        self.backends.entry(language.to_owned()).or_default();
        self.sessions.insert(language.to_owned(), session);
    }

    /// Run params through the interceptor chain if compression is enabled.
    async fn process_through_chain(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match &self.interceptor_chain {
            Some(chain) => match chain
                .process(method, params.clone(), Direction::ServerToClient)
                .await
            {
                Ok(Some(p)) => p,
                Ok(None) => serde_json::Value::Null,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        method = %method,
                        "Interceptor chain failed, returning original"
                    );
                    params
                }
            },
            None => params,
        }
    }

    /// Open a file in the given session.
    async fn open_file(
        session: &mut LspSession,
        uri: &str,
        language: &str,
    ) -> Result<String, anyhow::Error> {
        let path = uri
            .strip_prefix("file://")
            .ok_or_else(|| anyhow::anyhow!("URI must start with file://"))?;
        let content = tokio::fs::read_to_string(path).await?;
        session
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": language,
                        "version": 1,
                        "text": content,
                    }
                }),
            )
            .await?;
        Ok(content)
    }

    // ── Query methods ───────────────────────────────────────────────

    /// Get diagnostics for a file using the specified language's LSP server.
    pub async fn get_diagnostics(
        &mut self,
        uri: &str,
        language: &str,
    ) -> Result<String, anyhow::Error> {
        let session = self.session_for(language).await?;
        Self::open_file(session, uri, language).await?;

        let params = session
            .wait_for_notification("textDocument/publishDiagnostics")
            .await?;
        let processed = self
            .process_through_chain("textDocument/publishDiagnostics", params)
            .await;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Get completions at a cursor position using the specified language's LSP server.
    pub async fn get_completions(
        &mut self,
        uri: &str,
        language: &str,
        line: u32,
        character: u32,
    ) -> Result<String, anyhow::Error> {
        let session = self.session_for(language).await?;
        Self::open_file(session, uri, language).await?;

        let result = session
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
            .await;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    /// Get document symbols using the specified language's LSP server.
    pub async fn get_symbols(
        &mut self,
        uri: &str,
        language: &str,
    ) -> Result<String, anyhow::Error> {
        let session = self.session_for(language).await?;
        Self::open_file(session, uri, language).await?;

        let result = session
            .send_request(
                "textDocument/documentSymbol",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                }),
            )
            .await?;
        let processed = self
            .process_through_chain("textDocument/documentSymbol", result)
            .await;
        Ok(serde_json::to_string_pretty(&processed)?)
    }

    // ── Compression helpers ─────────────────────────────────────────

    /// Expand compressed diagnostics back to standard LSP format.
    pub fn inflate(compressed_json: &str) -> Result<String, anyhow::Error> {
        AgentHandle::inflate(compressed_json)
    }

    /// Compress standard diagnostics into compact format.
    pub fn compress(raw_json: &str) -> Result<String, anyhow::Error> {
        AgentHandle::compress(raw_json)
    }

    // ── Shutdown ─────────────────────────────────────────────────────

    /// Shut down all LSP server sessions.
    pub async fn shutdown_all(&mut self) -> Result<(), anyhow::Error> {
        for session in self.sessions.values_mut() {
            let _ = session
                .send_request("shutdown", serde_json::json!({}))
                .await;
            session
                .send_notification("exit", serde_json::json!({}))
                .await?;
        }
        self.sessions.clear();
        Ok(())
    }
}

/// Builder for [`AgentPool`].
#[derive(Default)]
pub struct AgentPoolBuilder {
    backends: Vec<(String, String)>,
    compression: bool,
}

impl AgentPoolBuilder {
    /// Register an LSP backend for a language identifier.
    pub fn register(mut self, language: impl Into<String>, backend: impl Into<String>) -> Self {
        self.backends.push((language.into(), backend.into()));
        self
    }

    /// Enable compression (default: disabled).
    pub fn enable_compression(mut self, enabled: bool) -> Self {
        self.compression = enabled;
        self
    }

    /// Spawn and initialize all registered backends, returning an [`AgentPool`].
    pub async fn start_all(self) -> Result<AgentPool, anyhow::Error> {
        let interceptor_chain = if self.compression {
            Some(build_pool_interceptor_chain())
        } else {
            None
        };

        let mut pool = AgentPool {
            sessions: HashMap::new(),
            backends: HashMap::new(),
            interceptor_chain,
        };

        for (language, backend) in &self.backends {
            pool.backends.insert(language.clone(), backend.clone());
        }

        Ok(pool)
    }
}

/// Build an interceptor chain with all compressors enabled (for AgentPool).
fn build_pool_interceptor_chain() -> InterceptorChain {
    use lspz_core::interceptors::completions::CompletionCompressor;
    use lspz_core::interceptors::diagnostics::DiagnosticsCompressor;
    use lspz_core::interceptors::hover::HoverCompressor;
    use lspz_core::interceptors::locations::LocationCompressor;
    use lspz_core::interceptors::symbols::DocumentSymbolCompressor;
    use lspz_core::interceptors::workspace_diagnostics::WorkspaceDiagnosticCompressor;
    use lspz_core::interceptors::workspace_symbols::WorkspaceSymbolCompressor;

    let config = Arc::new(RwLock::new(lspz_core::Config {
        backend_cmd: String::new(),
        capping: lspz_core::CappingConfig::default(),
        enable_diag_compress: true,
        enable_completion_compress: true,
        enable_hover_compress: true,
        enable_document_symbol_compress: true,
        enable_location_compress: true,
        enable_workspace_symbol_compress: true,
        enable_workspace_diag_compress: true,
        output_format: lspz_core::OutputFormat::Json,
        log_level: "info".into(),
        metrics: lspz_core::MetricsConfig::default(),
    }));

    let interceptors: Vec<Box<dyn Interceptor>> = vec![
        Box::new(DiagnosticsCompressor::default()),
        Box::new(CompletionCompressor::default()),
        Box::new(HoverCompressor::default()),
        Box::new(DocumentSymbolCompressor),
        Box::new(LocationCompressor),
        Box::new(WorkspaceSymbolCompressor),
        Box::new(WorkspaceDiagnosticCompressor),
    ];

    InterceptorChain::new(interceptors, config)
}

#[cfg(test)]
mod tests {
    use lspz_core::codec::json_rpc::LspMessage;
    use lspz_core::transport::mock::MockTransport;
    use lspz_mcp::LspSession;
    use serde_json::Value;

    use super::*;

    #[tokio::test]
    async fn test_builder_register_languages() {
        let pool = AgentPool::builder()
            .register("rust", "rust-analyzer")
            .register("go", "gopls")
            .start_all()
            .await
            .unwrap();

        assert_eq!(pool.backends.len(), 2);
        assert_eq!(pool.backends.get("rust").unwrap(), "rust-analyzer");
        assert_eq!(pool.backends.get("go").unwrap(), "gopls");
    }

    #[tokio::test]
    async fn test_get_diagnostics_unknown_language() {
        let mut pool = AgentPool::builder()
            .register("rust", "rust-analyzer")
            .start_all()
            .await
            .unwrap();

        let (uri, _path) = temp_file("fn main() {}");
        let err = pool.get_diagnostics(&uri, "python").await.unwrap_err();
        assert!(err.to_string().contains("no backend registered"), "{err}");
    }

    #[tokio::test]
    async fn test_insert_session_and_query() {
        let diag_notif = LspMessage::Notification {
            method: "textDocument/publishDiagnostics".into(),
            params: serde_json::json!({
                "uri": "file:///test.rs",
                "diagnostics": [{
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                    "severity": 1,
                    "message": "pool test",
                }],
            }),
        };

        let mock = MockTransport::new();
        mock.push_message(&diag_notif).unwrap();
        let session = LspSession::with_transport(Box::new(mock));

        let mut pool = AgentPool::builder()
            .register("rust", "rust-analyzer")
            .start_all()
            .await
            .unwrap();

        pool.insert_session("rust", session);

        let (uri, _path) = temp_file("fn main() {}");
        let result = pool.get_diagnostics(&uri, "rust").await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["diagnostics"][0]["message"], "pool test");
    }

    #[tokio::test]
    async fn test_multi_language_cross_talk() {
        let mock_a = MockTransport::new();
        mock_a
            .push_message(&LspMessage::Notification {
                method: "textDocument/publishDiagnostics".into(),
                params: serde_json::json!({
                    "uri": "file:///a.rs",
                    "diagnostics": [{
                        "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                        "severity": 1,
                        "message": "rust diag",
                    }],
                }),
            })
            .unwrap();
        let session_a = LspSession::with_transport(Box::new(mock_a));

        let mock_b = MockTransport::new();
        mock_b
            .push_message(&LspMessage::Notification {
                method: "textDocument/publishDiagnostics".into(),
                params: serde_json::json!({
                    "uri": "file:///b.py",
                    "diagnostics": [{
                        "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                        "severity": 2,
                        "message": "python diag",
                    }],
                }),
            })
            .unwrap();
        let session_b = LspSession::with_transport(Box::new(mock_b));

        let mut pool = AgentPool::builder()
            .register("rust", "rust-analyzer")
            .register("python", "basedpyright")
            .start_all()
            .await
            .unwrap();

        pool.insert_session("rust", session_a);
        pool.insert_session("python", session_b);

        let (uri_a, _) = temp_file("fn main() {}");
        let (uri_b, _) = temp_file("x = 1");

        let result_a = pool.get_diagnostics(&uri_a, "rust").await.unwrap();
        let parsed_a: Value = serde_json::from_str(&result_a).unwrap();
        assert_eq!(parsed_a["diagnostics"][0]["message"], "rust diag");

        let result_b = pool.get_diagnostics(&uri_b, "python").await.unwrap();
        let parsed_b: Value = serde_json::from_str(&result_b).unwrap();
        assert_eq!(parsed_b["diagnostics"][0]["message"], "python diag");
    }

    #[tokio::test]
    async fn test_inflate_compress() {
        let raw = serde_json::json!({
            "uri": "file:///test.rs",
            "diagnostics": []
        });
        let compact = AgentPool::compress(&raw.to_string()).unwrap();
        let expanded = AgentPool::inflate(&compact).unwrap();
        let parsed: Value = serde_json::from_str(&expanded).unwrap();
        assert_eq!(parsed["uri"], "file:///test.rs");
    }

    fn temp_file(content: &str) -> (String, String) {
        static COUNTER: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let path = format!("/tmp/lspz-pool-test-{id}.rs");
        std::fs::write(&path, content).unwrap();
        (format!("file://{path}"), path)
    }
}
