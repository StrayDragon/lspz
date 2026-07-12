use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::session::{InitializeParams, LspSession};

/// A pool of LSP sessions, keyed by `(language, workspace_root)`.
///
/// Sessions are created lazily on first access. Each session is behind its own
/// [`Mutex`] so LSP I/O on one session does not block other sessions once the
/// pool map lock is released.
pub struct LspPool {
    sessions: HashMap<String, Arc<Mutex<LspSession>>>,
}

impl LspPool {
    /// Create an empty pool.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Get or create a session without holding the pool mutex across `initialize`.
    ///
    /// Double-checked locking: lookup under a short lock, spawn+initialize outside
    /// the lock, then insert under a short lock (loser of a race is dropped).
    /// Dead child processes (try_wait reports exit) are removed before reuse.
    pub async fn get_or_spawn(
        pool: &Arc<Mutex<Self>>,
        language: &str,
        cmd: &str,
        root_path: Option<&str>,
        extra_args: &[String],
    ) -> Result<Arc<Mutex<LspSession>>, anyhow::Error> {
        let key = pool_key(language, cmd, root_path);
        {
            let mut guard = pool.lock().await;
            guard.reap_if_dead(&key);
            if let Some(session) = guard.sessions.get(&key) {
                return Ok(session.clone());
            }
        }

        let mut session = LspSession::spawn_with_args(cmd, extra_args)?;
        let init_params = InitializeParams {
            root_uri: root_path.map(|s| s.to_string()),
        };
        session.initialize(init_params).await?;
        tracing::info!(%key, "LSP session initialized");
        let session = Arc::new(Mutex::new(session));

        let mut guard = pool.lock().await;
        if let Some(existing) = guard.sessions.get(&key) {
            return Ok(existing.clone());
        }
        guard.sessions.insert(key, session.clone());
        Ok(session)
    }

    /// Insert `session` for `key` if absent; return the map entry (existing or new).
    pub fn insert_if_absent(
        &mut self,
        key: String,
        session: Arc<Mutex<LspSession>>,
    ) -> Arc<Mutex<LspSession>> {
        self.sessions.entry(key).or_insert(session).clone()
    }

    /// Look up an existing session by its pool key, reaping a dead child first.
    pub fn get_by_key(&mut self, key: &str) -> Result<Arc<Mutex<LspSession>>, anyhow::Error> {
        self.reap_if_dead(key);
        self.sessions
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no session for key: {key}"))
    }

    /// Remove `key` if its LSP child has exited (`try_wait` → `Some`).
    pub fn reap_if_dead(&mut self, key: &str) -> bool {
        let Some(session) = self.sessions.get(key).cloned() else {
            return false;
        };
        let Ok(mut guard) = session.try_lock() else {
            return false; // busy — leave for later
        };
        match guard.try_wait() {
            Ok(Some(status)) => {
                drop(guard);
                tracing::warn!(%key, ?status, "Removing dead LSP session");
                self.sessions.remove(key);
                true
            }
            _ => false,
        }
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
    /// Only reaps sessions that are not currently locked (try_lock). Busy
    /// sessions are skipped until a later pass.
    pub fn reap_idle(&mut self, idle_threshold: std::time::Duration) -> usize {
        let now = std::time::Instant::now();
        let before = self.sessions.len();
        self.sessions.retain(|_, s| match s.try_lock() {
            Ok(guard) => now.duration_since(guard.last_used_at()) < idle_threshold,
            Err(_) => true, // busy — keep
        });
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
    pub fn insert_session_for_test(&mut self, key: &str, session: LspSession) {
        self.sessions
            .insert(key.to_string(), Arc::new(Mutex::new(session)));
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

    #[test]
    fn reap_idle_zero_reaps_all() {
        let mut pool = make_pool(&["rust:rust-analyzer:/p", "go:gopls:/p"]);
        assert_eq!(pool.session_keys().len(), 2);

        let reaped = pool.reap_idle(Duration::ZERO);

        assert_eq!(reaped, 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn reap_idle_large_threshold_keeps_all() {
        let mut pool = make_pool(&["rust:rust-analyzer:/p"]);

        let reaped = pool.reap_idle(Duration::from_secs(3600));

        assert_eq!(reaped, 0);
        assert!(!pool.is_empty());
    }

    #[test]
    fn reap_idle_is_selective() {
        let mut pool = make_pool(&["a:x:/p", "b:y:/p"]);
        assert_eq!(pool.reap_idle(Duration::from_secs(3600)), 0);
        assert_eq!(pool.session_keys().len(), 2);

        assert_eq!(pool.reap_idle(Duration::ZERO), 2);
        assert!(pool.is_empty());
    }

    #[tokio::test]
    async fn get_by_key_returns_independent_arc() {
        let mut pool = make_pool(&["rust:ra:/p"]);
        let a = pool.get_by_key("rust:ra:/p").unwrap();
        let b = pool.get_by_key("rust:ra:/p").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[tokio::test]
    async fn get_or_spawn_reuses_existing_without_respawn() {
        let pool = Arc::new(Mutex::new(make_pool(&["rust:ra:/ws"])));
        let first = {
            let mut guard = pool.lock().await;
            guard.get_by_key("rust:ra:/ws").unwrap()
        };
        // Hit path: existing key must return same Arc (no initialize).
        let again = LspPool::get_or_spawn(&pool, "rust", "ra", Some("/ws"), &[])
            .await
            .unwrap();
        assert!(Arc::ptr_eq(&first, &again));
    }

    #[test]
    fn insert_if_absent_keeps_winner() {
        let mut pool = LspPool::new();
        let a = Arc::new(Mutex::new(LspSession::with_transport(Box::new(
            MockTransport::new(),
        ))));
        let b = Arc::new(Mutex::new(LspSession::with_transport(Box::new(
            MockTransport::new(),
        ))));
        let kept = pool.insert_if_absent("k".into(), a.clone());
        assert!(Arc::ptr_eq(&kept, &a));
        let kept2 = pool.insert_if_absent("k".into(), b);
        assert!(Arc::ptr_eq(&kept2, &a));
    }

    #[test]
    fn reap_if_dead_removes_exited_session() {
        let mock = MockTransport::new();
        mock.mark_exited();
        let mut pool = LspPool::new();
        pool.insert_session_for_test("rust:ra:/p", LspSession::with_transport(Box::new(mock)));
        assert!(pool.contains_key("rust:ra:/p"));
        assert!(pool.reap_if_dead("rust:ra:/p"));
        assert!(!pool.contains_key("rust:ra:/p"));
    }

    #[test]
    fn reap_if_dead_keeps_live_session() {
        let mut pool = make_pool(&["rust:ra:/p"]);
        assert!(!pool.reap_if_dead("rust:ra:/p"));
        assert!(pool.contains_key("rust:ra:/p"));
    }
}
