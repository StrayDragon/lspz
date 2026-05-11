//! Shared test utilities for E2E LSP server integration tests.
//!
//! Provides an [`LspTestHarness`] that spawns a real LSP server,
//! performs the initialize handshake, opens a test file, and
//! collects diagnostics. Tests gracefully skip if the server binary
//! is not installed on the current system.

use std::sync::atomic::{AtomicI64, Ordering};

use lspz_core::StdioTransport;
use lspz_core::Transport;
use lspz_core::codec::json_rpc::LspMessage;
use serde_json::Value;
use tempfile::TempDir;

/// A test harness for running real LSP servers.
pub struct LspTestHarness {
    pub transport: StdioTransport,
    pub temp_dir: TempDir,
    pub file_name: String,
    pub language_id: String,
    next_id: AtomicI64,
}

impl LspTestHarness {
    /// Try to start an LSP server for a given language.
    ///
    /// Returns `None` if the server binary is not on PATH.
    ///
    /// `cmd` — the server command (e.g. `"rust-analyzer"`).
    /// `file_content` — the source file content to open.
    /// `file_name` — the file name (used for URI construction).
    /// `language_id` — the LSP language identifier (e.g. `"rust"`).
    pub async fn try_new(
        cmd: &str,
        _file_content: &str,
        file_name: &str,
        language_id: &str,
    ) -> Option<Self> {
        // Check server availability
        let found = std::process::Command::new("which")
            .arg(cmd)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .is_some();
        if !found {
            eprintln!("  SKIP: '{}' not found on PATH", cmd);
            return None;
        }

        let temp_dir = TempDir::with_prefix("lspz_e2e_").ok()?;
        let transport = StdioTransport::spawn(cmd, &[]).ok()?;

        let mut harness = Self {
            transport,
            temp_dir,
            file_name: file_name.to_string(),
            language_id: language_id.to_string(),
            next_id: AtomicI64::new(1),
        };

        // Perform initialize handshake
        harness.initialize().await;

        Some(harness)
    }

    async fn send_request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = LspMessage::Request {
            id,
            method: method.into(),
            params,
        };
        let frame = msg.to_bytes().map_err(|e| e.to_string())?;
        self.transport
            .send(&frame)
            .await
            .map_err(|e| e.to_string())?;

        loop {
            let raw = self.transport.receive().await.map_err(|e| e.to_string())?;
            let parsed = LspMessage::from_frame_bytes(&raw).map_err(|e| e.to_string())?;
            match parsed {
                LspMessage::Response {
                    id: rid,
                    result,
                    error,
                    ..
                } if rid == id => {
                    if let Some(err) = error {
                        return Err(format!("LSP error {}: {}", err.code, err.message));
                    }
                    return Ok(result.unwrap_or(Value::Null));
                }
                LspMessage::Notification { method: m, .. } => {
                    tracing::trace!("Buffered notification: {}", m);
                }
                _ => {}
            }
        }
    }

    async fn send_notification(&mut self, method: &str, params: Value) -> Result<(), String> {
        let msg = LspMessage::Notification {
            method: method.into(),
            params,
        };
        let frame = msg.to_bytes().map_err(|e| e.to_string())?;
        self.transport.send(&frame).await.map_err(|e| e.to_string())
    }

    async fn initialize(&mut self) {
        let params = serde_json::json!({
            "processId": null,
            "capabilities": {},
            "rootUri": null,
            "workspaceFolders": null,
        });
        self.send_request("initialize", params)
            .await
            .expect("LSP initialize should succeed");
        self.send_notification("initialized", serde_json::json!({}))
            .await
            .expect("initialized notification should succeed");
    }

    /// Open a test file and return the file URI.
    pub fn write_source_file(&self, content: &str) -> String {
        let file_path = self.temp_dir.path().join(&self.file_name);
        std::fs::write(&file_path, content).expect("should write test file");
        format!("file://{}", file_path.display())
    }

    /// Send didOpen for the given URI and wait for publishDiagnostics.
    pub async fn get_diagnostics(&mut self, uri: &str) -> Result<Value, String> {
        let file_path = uri.strip_prefix("file://").unwrap();
        let content = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;

        let params = serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": self.language_id,
                "version": 1,
                "text": content,
            }
        });
        self.send_notification("textDocument/didOpen", params)
            .await?;

        // Wait for publishDiagnostics notification
        loop {
            let raw = self.transport.receive().await.map_err(|e| e.to_string())?;
            let parsed = LspMessage::from_frame_bytes(&raw).map_err(|e| e.to_string())?;
            match parsed {
                LspMessage::Notification { method, params }
                    if method == "textDocument/publishDiagnostics" =>
                {
                    return Ok(params);
                }
                LspMessage::Notification { method, .. } => {
                    tracing::trace!("Skipping notification: {}", method);
                }
                _ => {}
            }
        }
    }
}
