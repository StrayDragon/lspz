use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use lspz::StdioTransport;
use lspz::Transport;
use lspz::codec::json_rpc::LspMessage;
use serde_json::Value;
use tempfile::TempDir;

const LSP_TIMEOUT: Duration = Duration::from_secs(30);

pub struct LspTestHarness {
    pub transport: StdioTransport,
    pub temp_dir: TempDir,
    pub file_name: String,
    pub language_id: String,
    next_id: AtomicI64,
}

impl LspTestHarness {
    pub async fn try_new(
        cmd: &str,
        file_content: &str,
        file_name: &str,
        language_id: &str,
    ) -> Option<Self> {
        Self::try_new_with_args(cmd, &[], file_content, file_name, language_id).await
    }

    pub async fn try_new_with_args(
        cmd: &str,
        args: &[&str],
        file_content: &str,
        file_name: &str,
        language_id: &str,
    ) -> Option<Self> {
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
        let root_path = temp_dir.path().to_str()?.to_string();

        Self::scaffold_project(temp_dir.path(), language_id, file_name, file_content);

        let extra_args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let transport = StdioTransport::spawn(cmd, &extra_args).ok()?;

        let mut harness = Self {
            transport,
            temp_dir,
            file_name: file_name.to_string(),
            language_id: language_id.to_string(),
            next_id: AtomicI64::new(1),
        };

        if harness
            .initialize_with_root(Some(&root_path))
            .await
            .is_err()
        {
            eprintln!("  SKIP: '{}' initialize handshake failed", cmd);
            return None;
        }

        Some(harness)
    }

    fn scaffold_project(dir: &std::path::Path, lang: &str, file_name: &str, content: &str) {
        match lang {
            "rust" => {
                let src_dir = dir.join("src");
                let _ = std::fs::create_dir_all(&src_dir);
                let cargo_toml = dir.join("Cargo.toml");
                if !cargo_toml.exists() {
                    let _ = std::fs::write(
                        cargo_toml,
                        "[package]\nname = \"lspz_e2e\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                    );
                }
                let _ = std::fs::write(src_dir.join(file_name), content);
            }
            "go" => {
                let go_mod = dir.join("go.mod");
                if !go_mod.exists() {
                    let _ = std::fs::write(go_mod, "module lspz_e2e\ngo 1.21\n");
                }
                let _ = std::fs::write(dir.join(file_name), content);
            }
            "typescript" => {
                let _ = std::fs::write(dir.join(file_name), content);
            }
            _ => {
                let _ = std::fs::write(dir.join(file_name), content);
            }
        }
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
            let raw = tokio::time::timeout(LSP_TIMEOUT, self.transport.receive())
                .await
                .map_err(|_| format!("timeout waiting for response to '{method}'"))?
                .map_err(|e| e.to_string())?;
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

    async fn initialize_with_root(&mut self, root_uri: Option<&str>) -> Result<(), String> {
        let root_params =
            root_uri.map(|r| serde_json::json!([{"uri": format!("file://{r}"), "name": "root"}]));
        let params = serde_json::json!({
            "processId": null,
            "capabilities": {},
            "rootUri": root_uri.map(|r| format!("file://{r}")),
            "workspaceFolders": root_params,
        });
        self.send_request("initialize", params).await?;
        self.send_notification("initialized", serde_json::json!({}))
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn write_source_file(&self, content: &str) -> String {
        let file_path = match self.language_id.as_str() {
            "rust" => self.temp_dir.path().join("src").join(&self.file_name),
            _ => self.temp_dir.path().join(&self.file_name),
        };
        std::fs::write(&file_path, content).expect("should write test file");
        format!("file://{}", file_path.display())
    }

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

        loop {
            let raw = tokio::time::timeout(LSP_TIMEOUT, self.transport.receive())
                .await
                .map_err(|_| "timeout waiting for publishDiagnostics notification".to_string())?
                .map_err(|e| e.to_string())?;
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
