//! Daemon status — observable state of the lspz daemon.

use serde::{Deserialize, Serialize};

/// Per-session statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Pool key (e.g. "rust:rust-analyzer:/home/user/project").
    pub key: String,
    /// Language identifier.
    pub language: String,
    /// Backend command.
    pub backend: String,
    /// Workspace root path.
    pub workspace_root: Option<String>,
    /// Number of requests served by this session.
    pub request_count: u64,
    /// When the session was created (Unix timestamp seconds).
    pub created_at: u64,
    /// When the session was last used.
    pub last_used_at: u64,
}

/// Full daemon status snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    /// Total client connections served.
    pub total_connections: u64,
    /// Total requests dispatched.
    pub total_requests: u64,
    /// Uptime in seconds (refreshed on status snapshot).
    pub uptime_secs: u64,
    /// Per-session information.
    pub sessions: Vec<SessionInfo>,
    /// Process start time (Unix seconds). Skipped in JSON to keep the public
    /// snapshot shape stable; used only to compute [`Self::uptime_secs`].
    #[serde(skip)]
    started_at_unix: u64,
}

impl Default for DaemonStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl DaemonStatus {
    /// Create a status object stamped with the current start time.
    pub fn new() -> Self {
        Self {
            total_connections: 0,
            total_requests: 0,
            uptime_secs: 0,
            sessions: Vec::new(),
            started_at_unix: unix_now(),
        }
    }

    /// Refresh [`Self::uptime_secs`] from wall clock.
    pub fn refresh_uptime(&mut self) {
        let now = unix_now();
        self.uptime_secs = now.saturating_sub(self.started_at_unix);
    }

    /// Increment the lifetime connection counter.
    pub fn record_connection(&mut self) {
        self.total_connections = self.total_connections.saturating_add(1);
    }

    /// Increment the lifetime request counter.
    pub fn record_request(&mut self) {
        self.total_requests = self.total_requests.saturating_add(1);
    }

    /// Mark an existing session as just-used (looked up by pool key).
    ///
    /// Unlike [`touch_session`](Self::touch_session), this never creates a new
    /// entry — it only bumps `request_count` and `last_used_at` for an entry
    /// that [`touch_session`](Self::touch_session) already created. The daemon
    /// calls this when dispatching `lsp/request`, `lsp/notify`, and
    /// `lsp/wait_notify` so that the idle timestamps shown to users reflect
    /// real activity, not just the initial `lsp/spawn`.
    pub fn touch_by_key(&mut self, key: &str) {
        let now = unix_now();
        if let Some(s) = self.sessions.iter_mut().find(|s| s.key == key) {
            s.request_count += 1;
            s.last_used_at = now;
        }
    }

    /// Record or update a session entry.
    pub fn touch_session(
        &mut self,
        key: &str,
        language: &str,
        backend: &str,
        workspace_root: Option<String>,
    ) {
        let now = unix_now();

        if let Some(existing) = self.sessions.iter_mut().find(|s| s.key == key) {
            existing.request_count += 1;
            existing.last_used_at = now;
        } else {
            self.sessions.push(SessionInfo {
                key: key.to_string(),
                language: language.to_string(),
                backend: backend.to_string(),
                workspace_root,
                request_count: 1,
                created_at: now,
                last_used_at: now,
            });
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_counters_and_uptime() {
        let mut status = DaemonStatus::new();
        assert_eq!(status.total_connections, 0);
        assert_eq!(status.total_requests, 0);

        status.record_connection();
        status.record_request();
        status.record_request();
        status.refresh_uptime();

        assert_eq!(status.total_connections, 1);
        assert_eq!(status.total_requests, 2);
        // Just started — uptime may be 0 on the same second.
        assert!(status.uptime_secs < 60);
    }

    #[test]
    fn test_status_json_omits_started_at() {
        let status = DaemonStatus::new();
        let json = serde_json::to_value(&status).unwrap();
        assert!(json.get("started_at_unix").is_none());
        assert!(json.get("total_connections").is_some());
        assert!(json.get("uptime_secs").is_some());
    }
}
