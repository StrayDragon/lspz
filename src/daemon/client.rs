//! Daemon client — connects to a running lspz daemon.
//!
//! Provides auto-connect with transparent daemon auto-start:
//! if no daemon is running for the current workspace, the client spawns
//! a background daemon process and retries the connection.

use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{debug, info, warn};

use super::protocol::{
    DaemonRequest, DaemonResponse, LspNotifyParams, LspRequestParams, SpawnParams,
    SyncDocumentParams, WaitNotifyParams,
};
use super::socket::socket_path_for_workspace;

/// Maximum time to wait for daemon startup.
const DAEMON_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Client connection to a lspz daemon.
///
/// Manages the Unix socket connection and provides high-level
/// methods matching the daemon JSON-RPC protocol.
pub struct DaemonClient {
    #[allow(dead_code)]
    socket_path: PathBuf,
    /// Read half of the Unix socket.
    reader: BufReader<tokio::io::ReadHalf<UnixStream>>,
    /// Write half of the Unix socket.
    writer: tokio::io::WriteHalf<UnixStream>,
    next_id: u64,
    /// Whether this client is responsible for daemon lifecycle.
    owns_daemon: bool,
}

impl DaemonClient {
    /// Connect to an existing daemon or start one.
    ///
    /// `workspace_root` is a canonicalized absolute path.
    pub async fn connect_or_start(workspace_root: &str) -> Result<Self, anyhow::Error> {
        let socket_path = socket_path_for_workspace(workspace_root);

        // Try connecting to an existing daemon first.
        match UnixStream::connect(&socket_path).await {
            Ok(stream) => {
                info!(path = %socket_path.display(), "Connected to existing daemon");
                let (rh, wh) = tokio::io::split(stream);
                return Ok(Self {
                    socket_path,
                    reader: BufReader::new(rh),
                    writer: wh,
                    next_id: 1,
                    owns_daemon: false,
                });
            }
            Err(_) => {
                // Daemon not running — auto-start.
            }
        }

        // No daemon running — auto-start.
        info!(path = %socket_path.display(), "No daemon found, auto-starting");
        spawn_background_daemon(&socket_path, workspace_root)?;

        // Wait for daemon to be ready (retry with backoff).
        let start = std::time::Instant::now();
        while start.elapsed() < DAEMON_STARTUP_TIMEOUT {
            tokio::time::sleep(Duration::from_millis(200)).await;
            match UnixStream::connect(&socket_path).await {
                Ok(stream) => {
                    info!("Connected to freshly spawned daemon");
                    let (rh, wh) = tokio::io::split(stream);
                    return Ok(Self {
                        socket_path,
                        reader: BufReader::new(rh),
                        writer: wh,
                        next_id: 1,
                        owns_daemon: true,
                    });
                }
                Err(e)
                    if e.kind() == ErrorKind::ConnectionRefused
                        || e.kind() == ErrorKind::NotFound =>
                {
                    continue;
                }
                Err(e) => {
                    warn!(error = %e, "Unexpected error connecting to daemon");
                    continue;
                }
            }
        }

        anyhow::bail!("Daemon failed to start within {DAEMON_STARTUP_TIMEOUT:?}");
    }

    /// Connect to a daemon at a specific socket path (for testing).
    pub async fn connect_explicit(socket: &PathBuf) -> Result<Self, anyhow::Error> {
        let stream = UnixStream::connect(socket)
            .await
            .with_context(|| format!("Cannot connect to daemon at {:?}", socket))?;
        let (rh, wh) = tokio::io::split(stream);
        Ok(Self {
            socket_path: socket.clone(),
            reader: BufReader::new(rh),
            writer: wh,
            next_id: 1,
            owns_daemon: false,
        })
    }

    /// Check if daemon is already running (without auto-start).
    pub async fn try_connect(workspace_root: &str) -> Result<Option<Self>, anyhow::Error> {
        let socket_path = socket_path_for_workspace(workspace_root);
        match UnixStream::connect(&socket_path).await {
            Ok(stream) => {
                let (rh, wh) = tokio::io::split(stream);
                Ok(Some(Self {
                    socket_path,
                    reader: BufReader::new(rh),
                    writer: wh,
                    next_id: 1,
                    owns_daemon: false,
                }))
            }
            Err(_) => Ok(None),
        }
    }

    /// Get or create an LSP session on the daemon.
    pub async fn spawn_session(&mut self, params: &SpawnParams) -> Result<String, anyhow::Error> {
        let resp = self
            .request("lsp/spawn", &serde_json::to_value(params)?)
            .await?;
        if let Some(err) = resp.error {
            anyhow::bail!("spawn failed: {err}");
        }
        Ok(resp.result["session_key"]
            .as_str()
            .context("missing session_key")?
            .to_string())
    }

    /// Send an LSP request through a session and await the response.
    pub async fn lsp_request(
        &mut self,
        session_key: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, anyhow::Error> {
        let req = LspRequestParams {
            session_key: session_key.to_string(),
            method: method.to_string(),
            params,
        };
        let resp = self
            .request("lsp/request", &serde_json::to_value(&req)?)
            .await?;
        if let Some(err) = resp.error {
            anyhow::bail!("LSP request '{}' failed: {err}", method);
        }
        Ok(resp.result)
    }

    /// Send an LSP notification through a session.
    pub async fn lsp_notify(
        &mut self,
        session_key: &str,
        method: &str,
        params: Value,
    ) -> Result<(), anyhow::Error> {
        let req = LspNotifyParams {
            session_key: session_key.to_string(),
            method: method.to_string(),
            params,
        };
        let resp = self
            .request("lsp/notify", &serde_json::to_value(&req)?)
            .await?;
        if let Some(err) = resp.error {
            anyhow::bail!("LSP notify '{}' failed: {err}", method);
        }
        Ok(())
    }

    /// Wait for a notification from the LSP server.
    ///
    /// `timeout_ms`, when set, is forwarded to the daemon, which enforces the
    /// deadline and always writes a response before it elapses. This avoids the
    /// client cancelling a still-running wait (which would orphan the response
    /// line). It should be comfortably shorter than the internal read timeout
    /// used by the request/response loop.
    pub async fn lsp_wait_notify(
        &mut self,
        session_key: &str,
        method: &str,
        filter_uri: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<Value, anyhow::Error> {
        let req = WaitNotifyParams {
            session_key: session_key.to_string(),
            method: method.to_string(),
            filter_uri: filter_uri.map(|s| s.to_string()),
            timeout_ms,
        };
        let resp = self
            .request("lsp/wait_notify", &serde_json::to_value(&req)?)
            .await?;
        if let Some(err) = resp.error {
            anyhow::bail!("wait_notify '{}' failed: {err}", method);
        }
        Ok(resp.result)
    }

    /// Open or update a document via [`crate::mcp::LspSession::open_or_update_document`].
    pub async fn lsp_sync_document(
        &mut self,
        session_key: &str,
        uri: &str,
        language_id: &str,
        content: &str,
    ) -> Result<(), anyhow::Error> {
        let req = SyncDocumentParams {
            session_key: session_key.to_string(),
            uri: uri.to_string(),
            language_id: language_id.to_string(),
            content: content.to_string(),
        };
        let resp = self
            .request("lsp/sync_document", &serde_json::to_value(&req)?)
            .await?;
        if let Some(err) = resp.error {
            anyhow::bail!("LSP sync_document failed: {err}");
        }
        Ok(())
    }

    /// Query daemon status (sessions, stats).
    pub async fn get_status(&mut self) -> Result<Value, anyhow::Error> {
        let resp = self
            .request("daemon/status", &serde_json::json!({}))
            .await?;
        if let Some(err) = resp.error {
            anyhow::bail!("status failed: {err}");
        }
        Ok(resp.result)
    }

    /// Gracefully shut down the daemon.
    pub async fn shutdown(&mut self) -> Result<(), anyhow::Error> {
        let _ = self
            .request("daemon/shutdown", &serde_json::json!({}))
            .await;
        Ok(())
    }

    // ─── Internal ────────────────────────────────────────────────

    /// Send a JSON-RPC request and read the matching response.
    ///
    /// The daemon protocol is newline-delimited JSON with request/response
    /// correlation by `id`. Responses are **not** guaranteed to arrive in the
    /// exact order the client expects: an earlier request whose future was
    /// cancelled (e.g. a tool call dropped by the caller) still gets completed
    /// by the daemon, whose response line then sits on the wire as an "orphan".
    ///
    /// Reading exactly one line and returning it would mistake that orphan for
    /// *this* request's response, permanently desyncing the stream — every
    /// later request would read the wrong line (this is the root cause of the
    /// `spawn failed: Wait for notification failed: ...` and `missing 'uri' in
    /// publishDiagnostics` errors). We therefore loop, draining any
    /// orphan/out-of-order lines (logging them), until the response whose `id`
    /// matches ours arrives.
    async fn request(
        &mut self,
        method: &str,
        params: &Value,
    ) -> Result<DaemonResponse, anyhow::Error> {
        let id = self.next_id;
        self.next_id += 1;

        let req = DaemonRequest {
            id,
            method: method.to_string(),
            params: params.clone(),
        };
        let json = format!("{}\n", serde_json::to_string(&req)?);

        // Write request.
        self.writer.write_all(json.as_bytes()).await?;
        self.writer.flush().await?;

        // Read response lines until ours arrives, draining orphans.
        loop {
            let mut line = String::new();
            tokio::time::timeout(Duration::from_secs(30), self.reader.read_line(&mut line))
                .await
                .context("Timeout waiting for daemon response")?
                .context("Daemon connection closed")?;

            let resp: DaemonResponse = serde_json::from_str(line.trim())?;
            if resp.id == id {
                return Ok(resp);
            }
            warn!(
                expected = id,
                got = resp.id,
                error = ?resp.error,
                "Drained orphan/out-of-order daemon response from a cancelled or \
                 earlier request; protocol stays in sync",
            );
        }
    }
}

impl Drop for DaemonClient {
    fn drop(&mut self) {
        if !self.owns_daemon {
            debug!("DaemonClient dropped (owns_daemon=false)");
            return;
        }
        let socket = self.socket_path.clone();
        debug!(
            ?socket,
            "DaemonClient dropped (owns_daemon=true); requesting shutdown"
        );
        // Best-effort: Drop cannot await. Open a fresh connection on a helper
        // thread so we do not block the runtime that may still own this client.
        std::thread::Builder::new()
            .name("lspz-daemon-shutdown".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        warn!(error = %e, "Failed to build runtime for daemon shutdown");
                        return;
                    }
                };
                rt.block_on(async {
                    match DaemonClient::connect_explicit(&socket).await {
                        Ok(mut client) => {
                            if let Err(e) = client.shutdown().await {
                                warn!(error = %e, "daemon/shutdown after Drop failed");
                            }
                        }
                        Err(e) => {
                            debug!(error = %e, "Could not connect to shut down owned daemon");
                        }
                    }
                });
            })
            .ok();
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────

/// Spawn a background daemon process for the given workspace.
fn spawn_background_daemon(
    socket_path: &PathBuf,
    workspace_root: &str,
) -> Result<(), anyhow::Error> {
    let exe = std::env::current_exe().context("Could not determine lspz binary path")?;

    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("daemon")
        .arg("--socket")
        .arg(socket_path)
        .env("LSPZ_DAEMON_WORKSPACE", workspace_root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // Detach from the parent process (Unix-only).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                // Create new process group to fully detach
                libc::setsid();
                Ok(())
            });
        }
    }

    let _child = cmd.spawn().context("Failed to spawn lspz daemon process")?;

    Ok(())
}
