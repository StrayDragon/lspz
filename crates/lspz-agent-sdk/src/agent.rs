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

impl AgentHandle {
    /// Create a new [`AgentBuilder`].
    pub fn builder() -> AgentBuilder {
        AgentBuilder::default()
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
    pub fn inflate(&self, compressed_json: &str) -> Result<String, anyhow::Error> {
        let compressed: Value = serde_json::from_str(compressed_json)?;
        let expanded = compact::decompress(&compressed)?;
        Ok(serde_json::to_string_pretty(&expanded)?)
    }

    /// Compress standard diagnostics into compact format.
    ///
    /// Takes a standard `publishDiagnostics` params JSON string and
    /// compresses it into lspz's compact format.
    pub fn compress(&self, raw_json: &str) -> Result<String, anyhow::Error> {
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
