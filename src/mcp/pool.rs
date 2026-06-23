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
    /// `root_path` is a canonicalized absolute path (no `file://` prefix) used
    /// both as part of the cache key and as the LSP `rootUri` during initialization.
    /// `extra_args` are additional CLI arguments passed when spawning a new session.
    pub async fn get_or_spawn(
        &mut self,
        language: &str,
        cmd: &str,
        root_path: Option<&str>,
        extra_args: &[String],
    ) -> Result<&mut LspSession, anyhow::Error> {
        let key = pool_key(language, cmd, root_path);
        if !self.sessions.contains_key(&key) {
            let mut session = LspSession::spawn_with_args(cmd, extra_args)?;
            let init_params = InitializeParams {
                root_uri: root_path.map(|s| s.to_string()),
            };
            session.initialize(init_params).await?;
            tracing::info!(key, "LSP session initialized");
            self.sessions.insert(key.clone(), session);
        }
        Ok(self.sessions.get_mut(&key).unwrap())
    }

    /// Look up an existing session by its pool key.
    ///
    /// Returns an error if the session hasn't been spawned yet.
    /// The key format is `{language}:{backend}:{root_path}` or `{language}:{backend}`.
    pub fn get_mut_by_key(&mut self, key: &str) -> Result<&mut LspSession, anyhow::Error> {
        self.sessions
            .get_mut(key)
            .ok_or_else(|| anyhow::anyhow!("no session for key: {key}"))
    }

    /// Check whether a session exists for the given key.
    pub fn contains_key(&self, key: &str) -> bool {
        self.sessions.contains_key(key)
    }

    /// Snapshot current session keys for status reporting.
    pub fn session_keys(&self) -> Vec<String> {
        self.sessions.keys().cloned().collect()
    }
}

#[cfg(test)]
impl LspPool {
    /// Insert a pre-built session under a key (testing only).
    ///
    /// Allows driving daemon handlers against a session backed by a mock
    /// transport, without spawning a real LSP server.
    pub fn insert_session_for_test(&mut self, key: &str, session: LspSession) {
        self.sessions.insert(key.to_string(), session);
    }
}

impl Default for LspPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a pool key from language, backend command, and optional workspace root.
pub fn pool_key(language: &str, cmd: &str, root_path: Option<&str>) -> String {
    match root_path {
        Some(root) => format!("{language}:{cmd}:{root}"),
        None => format!("{language}:{cmd}"),
    }
}
