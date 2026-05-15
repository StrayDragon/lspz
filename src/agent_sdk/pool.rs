//! AgentPool — multi-language LSP session management for AI agents.

use std::collections::HashMap;

use super::AgentHandle;

/// A pool of LSP sessions managed by language identifier.
///
/// Sessions are lazily spawned on the first query for a given language.
/// All operations delegate to [`AgentHandle`] instances internally.
pub struct AgentPool {
    handles: HashMap<String, AgentHandle>,
    backends: HashMap<String, BackendConfig>,
    compression: bool,
}

struct BackendConfig {
    backend: String,
    workspace_root: Option<String>,
}

impl std::fmt::Debug for AgentPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentPool")
            .field("languages", &self.backends.keys().collect::<Vec<_>>())
            .field("active_handles", &self.handles.len())
            .field("compression", &self.compression)
            .finish()
    }
}

impl AgentPool {
    /// Create a new [`AgentPoolBuilder`].
    pub fn builder() -> AgentPoolBuilder {
        AgentPoolBuilder::default()
    }

    async fn handle_for(&mut self, language: &str) -> Result<&mut AgentHandle, anyhow::Error> {
        if !self.handles.contains_key(language) {
            let cfg = self.backends.get(language).ok_or_else(|| {
                anyhow::anyhow!("no backend registered for language '{language}'")
            })?;
            let mut builder = AgentHandle::builder()
                .backend(&cfg.backend)
                .language(language)
                .enable_compression(self.compression);
            if let Some(root) = &cfg.workspace_root {
                builder = builder.workspace_root(root);
            }
            let handle = builder.start().await?;
            tracing::info!(language, "LSP handle created via pool");
            self.handles.insert(language.to_owned(), handle);
        }
        Ok(self.handles.get_mut(language).unwrap())
    }

    /// Insert a pre-constructed [`AgentHandle`] for testing.
    #[allow(dead_code)]
    pub(crate) fn insert_handle(&mut self, language: &str, handle: AgentHandle) {
        self.backends
            .entry(language.to_owned())
            .or_insert_with(|| BackendConfig {
                backend: String::new(),
                workspace_root: None,
            });
        self.handles.insert(language.to_owned(), handle);
    }

    // ── Query methods (delegation to AgentHandle) ─────────────────────

    /// Get diagnostics for a file using the specified language's LSP server.
    pub async fn get_diagnostics(
        &mut self,
        uri: &str,
        language: &str,
    ) -> Result<String, anyhow::Error> {
        self.handle_for(language).await?.get_diagnostics(uri).await
    }

    /// Get completions at a cursor position.
    pub async fn get_completions(
        &mut self,
        uri: &str,
        language: &str,
        line: u32,
        character: u32,
    ) -> Result<String, anyhow::Error> {
        self.handle_for(language)
            .await?
            .get_completions(uri, line, character)
            .await
    }

    /// Get document symbols using the specified language's LSP server.
    pub async fn get_symbols(
        &mut self,
        uri: &str,
        language: &str,
    ) -> Result<String, anyhow::Error> {
        self.handle_for(language).await?.get_symbols(uri).await
    }

    // ── Refactoring operations ──────────────────────────────────────────

    /// Rename a symbol.
    pub async fn rename(
        &mut self,
        uri: &str,
        language: &str,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<String, anyhow::Error> {
        self.handle_for(language)
            .await?
            .rename(uri, line, character, new_name)
            .await
    }

    /// Get code actions.
    pub async fn code_action(
        &mut self,
        uri: &str,
        language: &str,
        line: u32,
        character: u32,
        diagnostics: Option<Vec<serde_json::Value>>,
        only: Option<Vec<String>>,
    ) -> Result<String, anyhow::Error> {
        self.handle_for(language)
            .await?
            .code_action(uri, line, character, diagnostics, only)
            .await
    }

    /// Format a file.
    pub async fn formatting(
        &mut self,
        uri: &str,
        language: &str,
        options: Option<serde_json::Value>,
    ) -> Result<String, anyhow::Error> {
        self.handle_for(language)
            .await?
            .formatting(uri, options)
            .await
    }

    // ── Raw request ──────────────────────────────────────────────────

    /// Send a raw LSP request and return the response as a JSON string.
    pub async fn send_raw(
        &mut self,
        language: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<String, anyhow::Error> {
        self.handle_for(language)
            .await?
            .send_raw(method, params)
            .await
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

    // ── File sync notifications ──────────────────────────────────────

    /// Notify the LSP server that a file's content has changed.
    pub async fn notify_change(
        &mut self,
        uri: &str,
        language: &str,
        content: &str,
    ) -> Result<(), anyhow::Error> {
        self.handle_for(language)
            .await?
            .notify_change(uri, content)
            .await
    }

    /// Notify the LSP server that a file has been closed.
    pub async fn notify_close(&mut self, uri: &str, language: &str) -> Result<(), anyhow::Error> {
        self.handle_for(language).await?.notify_close(uri).await
    }

    /// Notify the LSP server that a file has been saved.
    pub async fn notify_save(&mut self, uri: &str, language: &str) -> Result<(), anyhow::Error> {
        self.handle_for(language).await?.notify_save(uri).await
    }

    // ── Shutdown ─────────────────────────────────────────────────────

    /// Shut down all LSP server sessions.
    pub async fn shutdown_all(self) -> Result<(), anyhow::Error> {
        for (language, handle) in self.handles {
            if let Err(e) = handle.shutdown().await {
                tracing::warn!(language, error = %e, "Failed to shutdown handle");
            }
        }
        Ok(())
    }
}

/// Builder for [`AgentPool`].
#[derive(Default)]
pub struct AgentPoolBuilder {
    backends: Vec<(String, String)>,
    workspace_root: Option<String>,
    compression: bool,
}

impl AgentPoolBuilder {
    /// Register an LSP backend for a language identifier.
    pub fn register(mut self, language: impl Into<String>, backend: impl Into<String>) -> Self {
        self.backends.push((language.into(), backend.into()));
        self
    }

    /// Set a shared workspace root for all registered backends.
    pub fn workspace_root(mut self, path: impl Into<String>) -> Self {
        self.workspace_root = Some(path.into());
        self
    }

    /// Enable compression (default: disabled).
    pub fn enable_compression(mut self, enabled: bool) -> Self {
        self.compression = enabled;
        self
    }

    /// Create an [`AgentPool`] with registered backends.
    ///
    /// Sessions are **not** spawned eagerly — they are created lazily on first
    /// query via the internal language dispatcher.
    pub async fn start_all(self) -> Result<AgentPool, anyhow::Error> {
        let mut backends = HashMap::new();
        for (language, backend) in &self.backends {
            backends.insert(
                language.clone(),
                BackendConfig {
                    backend: backend.clone(),
                    workspace_root: self.workspace_root.clone(),
                },
            );
        }

        Ok(AgentPool {
            handles: HashMap::new(),
            backends,
            compression: self.compression,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::json_rpc::LspMessage;
    use crate::mcp::LspSession;
    use crate::transport::mock::MockTransport;
    use serde_json::Value;

    use super::*;

    fn mock_handle(responses: Vec<LspMessage>) -> AgentHandle {
        let mock = MockTransport::new();
        for msg in responses {
            mock.push_message(&msg).unwrap();
        }
        let session = LspSession::with_transport(Box::new(mock));
        AgentHandle::new(session, "rust".into(), false)
    }

    #[tokio::test]
    async fn test_builder_register_languages() {
        let pool = AgentPool::builder()
            .register("rust", "rust-analyzer")
            .register("go", "gopls")
            .start_all()
            .await
            .unwrap();

        assert_eq!(pool.backends.len(), 2);
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
    async fn test_insert_handle_and_query() {
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

        let handle = mock_handle(vec![diag_notif]);

        let mut pool = AgentPool::builder()
            .register("rust", "rust-analyzer")
            .start_all()
            .await
            .unwrap();

        pool.insert_handle("rust", handle);

        let (uri, _path) = temp_file("fn main() {}");
        let result = pool.get_diagnostics(&uri, "rust").await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["diagnostics"][0]["message"], "pool test");
    }

    #[tokio::test]
    async fn test_multi_language_cross_talk() {
        let handle_a = {
            let mock = MockTransport::new();
            mock.push_message(&LspMessage::Notification {
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
            let session = LspSession::with_transport(Box::new(mock));
            AgentHandle::new(session, "rust".into(), false)
        };

        let handle_b = {
            let mock = MockTransport::new();
            mock.push_message(&LspMessage::Notification {
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
            let session = LspSession::with_transport(Box::new(mock));
            AgentHandle::new(session, "python".into(), false)
        };

        let mut pool = AgentPool::builder()
            .register("rust", "rust-analyzer")
            .register("python", "basedpyright")
            .start_all()
            .await
            .unwrap();

        pool.insert_handle("rust", handle_a);
        pool.insert_handle("python", handle_b);

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
