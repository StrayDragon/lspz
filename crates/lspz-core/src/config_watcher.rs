//! File-system watcher for hot-reloading [`Config`](crate::config::Config).
//!
//! Watches a TOML config file for changes and atomically swaps the shared config.

use std::path::PathBuf;
use std::sync::Arc;

use notify::event::EventKind;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::RwLock;

use crate::config::Config as LspzConfig;

/// File-system watcher for hot-reloading [`LspzConfig`].
///
/// Spawns a background tokio task that monitors the config file.
/// On every change, re-reads the TOML and updates the shared `Arc<RwLock<LspzConfig>>`.
pub struct ConfigWatcher {
    _watcher: RecommendedWatcher,
    task: tokio::task::JoinHandle<()>,
}

impl ConfigWatcher {
    /// Start watching `path` for changes, writing updates into `shared_config`.
    ///
    /// Returns immediately; the watcher runs in the background.
    pub fn spawn(
        path: impl Into<PathBuf>,
        shared_config: Arc<RwLock<LspzConfig>>,
    ) -> Result<Self, crate::error::LspzError> {
        let path: PathBuf = path.into();
        let config_path = path.clone();

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Some(event) = res
                    .ok()
                    .filter(|e| !matches!(e.kind, EventKind::Access(_) | EventKind::Other))
                {
                    let _ = tx.try_send(event);
                }
            },
            notify::Config::default(),
        )
        .map_err(|e| crate::error::LspzError::Config(format!("notify watcher failed: {e}")))?;

        watcher
            .watch(&config_path, RecursiveMode::NonRecursive)
            .map_err(|e| {
                crate::error::LspzError::Config(format!("failed to watch config file: {e}"))
            })?;

        tracing::info!(path = %config_path.display(), "Config watcher started");

        let task = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    tracing::debug!("Config file changed, reloading...");
                    if let Err(e) = Self::reload(&config_path, &shared_config).await {
                        tracing::warn!(error = %e, "Config reload failed, keeping previous config");
                    }
                }
            }
        });

        Ok(Self {
            _watcher: watcher,
            task,
        })
    }

    /// Abort the background watcher task.
    pub fn shutdown(self) {
        self.task.abort();
    }

    /// Re-read the TOML file and update the shared config.
    async fn reload(
        path: &PathBuf,
        shared: &Arc<RwLock<LspzConfig>>,
    ) -> Result<(), crate::error::LspzError> {
        match LspzConfig::from_file(path) {
            Ok(new_config) => {
                let mut guard = shared.write().await;
                *guard = new_config;
                tracing::info!("Config reloaded from {}", path.display());
                Ok(())
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to reload config, keeping previous");
                Err(e)
            }
        }
    }
}
