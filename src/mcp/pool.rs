use std::collections::HashMap;

use super::session::{InitializeParams, LspSession};

/// A pool of LSP sessions, keyed by a user-defined name (typically a language
/// identifier like `"go"` or `"rust"`).
///
/// Sessions are created lazily on first access.
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

    /// Get or create a session for the given key.
    pub async fn get_or_spawn(
        &mut self,
        key: &str,
        cmd: &str,
    ) -> Result<&mut LspSession, anyhow::Error> {
        if !self.sessions.contains_key(key) {
            let mut session = LspSession::spawn(cmd)?;
            session.initialize(InitializeParams::default()).await?;
            tracing::info!(key, "LSP session initialized");
            self.sessions.insert(key.into(), session);
        }
        Ok(self.sessions.get_mut(key).unwrap())
    }
}

impl Default for LspPool {
    fn default() -> Self {
        Self::new()
    }
}
