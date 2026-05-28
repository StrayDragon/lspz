use std::collections::HashMap;

use super::session::{InitializeParams, LspSession};

/// A pool of LSP sessions, keyed by `(language, workspace_root)`.
///
/// Sessions are created lazily on first access. Different workspace roots
/// get separate sessions so each LSP server analyzes the correct project.
pub struct LspPool {
    sessions: HashMap<String, LspSession>,
}

impl LspPool {
    /// Create an empty pool.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Get or create a session for the given language and workspace root.
    ///
    /// `root_uri` is used as the LSP `rootUri` during initialization so the
    /// server knows which project to analyze. Sessions are keyed by
    /// `"{language}:{root_uri}"` to isolate different projects.
    pub async fn get_or_spawn(
        &mut self,
        language: &str,
        cmd: &str,
        root_uri: Option<&str>,
    ) -> Result<&mut LspSession, anyhow::Error> {
        let key = match root_uri {
            Some(root) => format!("{language}:{root}"),
            None => language.to_string(),
        };
        if !self.sessions.contains_key(&key) {
            let mut session = LspSession::spawn(cmd)?;
            let init_params = InitializeParams {
                root_uri: root_uri.map(|s| s.to_string()),
            };
            session.initialize(init_params).await?;
            tracing::info!(key, "LSP session initialized");
            self.sessions.insert(key.clone(), session);
        }
        Ok(self.sessions.get_mut(&key).unwrap())
    }
}

impl Default for LspPool {
    fn default() -> Self {
        Self::new()
    }
}
