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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonStatus {
    /// Total client connections served.
    pub total_connections: u64,
    /// Total requests dispatched.
    pub total_requests: u64,
    /// Uptime in seconds.
    pub uptime_secs: u64,
    /// Per-session information.
    pub sessions: Vec<SessionInfo>,
}

impl DaemonStatus {
    /// Record or update a session entry.
    pub fn touch_session(
        &mut self,
        key: &str,
        language: &str,
        backend: &str,
        workspace_root: Option<String>,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

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
