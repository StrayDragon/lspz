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

    /// Remove sessions whose last I/O was longer ago than `idle_threshold`.
    ///
    /// Dropping an [`LspSession`] terminates its child LSP server process, so
    /// this reclaims the memory and CPU held by stale language servers. The
    /// daemon calls this periodically to keep idle workspaces from piling up.
    ///
    /// Returns the number of sessions removed.
    pub fn reap_idle(&mut self, idle_threshold: std::time::Duration) -> usize {
        let now = std::time::Instant::now();
        let before = self.sessions.len();
        self.sessions
            .retain(|_, s| now.duration_since(s.last_used_at()) < idle_threshold);
        before - self.sessions.len()
    }

    /// Remove all sessions, dropping their transports (kills child LSP processes).
    pub fn clear(&mut self) {
        let n = self.sessions.len();
        self.sessions.clear();
        if n > 0 {
            tracing::info!(cleared = n, "Cleared all LSP sessions");
        }
    }

    /// Returns `true` if the pool holds no sessions.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::MockTransport;
    use std::time::Duration;

    fn make_pool(keys: &[&str]) -> LspPool {
        let mut pool = LspPool::new();
        for k in keys {
            pool.insert_session_for_test(
                k,
                LspSession::with_transport(Box::new(MockTransport::new())),
            );
        }
        pool
    }

    #[test]
    fn is_empty_for_fresh_pool() {
        assert!(LspPool::new().is_empty());
    }

    #[test]
    fn clear_removes_all_sessions() {
        let mut pool = make_pool(&["rust:rust-analyzer:/p", "go:gopls:/p"]);
        assert_eq!(pool.session_keys().len(), 2);
        pool.clear();
        assert!(pool.is_empty());
    }

    /// A zero threshold reaps everything immediately, since `now - last_used`
    /// is never less than zero. Sessions are freshly created so their
    /// `last_used_at` equals `now`, making `0 < 0` false → reaped.
    #[test]
    fn reap_idle_zero_reaps_all() {
        let mut pool = make_pool(&["rust:rust-analyzer:/p", "go:gopls:/p"]);
        assert_eq!(pool.session_keys().len(), 2);

        let reaped = pool.reap_idle(Duration::ZERO);

        assert_eq!(reaped, 2);
        assert!(pool.is_empty());
    }

    /// A large threshold keeps freshly-created sessions (their last I/O is
    /// essentially *now*, well within the window).
    #[test]
    fn reap_idle_large_threshold_keeps_all() {
        let mut pool = make_pool(&["rust:rust-analyzer:/p"]);

        let reaped = pool.reap_idle(Duration::from_secs(3600));

        assert_eq!(reaped, 0);
        assert!(!pool.is_empty());
    }

    /// Reaping is selective: only sessions past the threshold are dropped.
    #[test]
    fn reap_idle_is_selective() {
        // Two sessions; both fresh, so a zero threshold reaps both while a
        // large threshold keeps both. (We can't fast-forward `Instant`, so we
        // verify the boundary behavior from both sides instead.)
        let mut pool = make_pool(&["a:x:/p", "b:y:/p"]);
        assert_eq!(pool.reap_idle(Duration::from_secs(3600)), 0);
        assert_eq!(pool.session_keys().len(), 2);

        assert_eq!(pool.reap_idle(Duration::ZERO), 2);
        assert!(pool.is_empty());
    }
}
