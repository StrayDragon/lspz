//! Daemon server — long-lived LSP session manager.
//!
//! Accepts connections on a Unix domain socket, multiplexes LSP requests
//! over the internal [`LspPool`], and returns results as JSON-RPC responses.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use super::protocol::DaemonResponse;
use super::status::DaemonStatus;
use crate::mcp::LspPool;

/// The daemon server.
pub struct DaemonServer {
    socket_path: PathBuf,
    pool: Arc<Mutex<LspPool>>,
    status: Arc<Mutex<DaemonStatus>>,
}

impl DaemonServer {
    /// Create a new daemon server.
    ///
    /// `workspace_root` should be a canonicalized absolute path uniquely
    /// identifying the project workspace.
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            pool: Arc::new(Mutex::new(LspPool::new())),
            status: Arc::new(Mutex::new(DaemonStatus::default())),
        }
    }

    /// Start the daemon, listening on the Unix socket.
    ///
    /// This spawns a cleanup task on SIGTERM/SIGINT.
    /// Returns immediately (the daemon runs until signalled).
    pub async fn start(self) -> Result<(), anyhow::Error> {
        // Remove stale socket file
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).with_context(|| {
                format!("Failed to remove stale socket: {:?}", self.socket_path)
            })?;
        }

        // Ensure parent directory exists
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&self.socket_path)
            .with_context(|| format!("Failed to bind to {:?}", self.socket_path))?;

        let socket_path = self.socket_path.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            info!("Daemon received shutdown signal");
            let _ = std::fs::remove_file(&socket_path);
            std::process::exit(0);
        });

        info!(path = %self.socket_path.display(), "Daemon listening");

        let pool = self.pool.clone();
        let status = self.status.clone();

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    debug!("Daemon: new client connection");
                    let pool = pool.clone();
                    let status = status.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, pool, status).await {
                            warn!(error = %e, "Client handler exited with error");
                        }
                    });
                }
                Err(e) => {
                    error!(error = %e, "Daemon accept error");
                }
            }
        }
    }
}

/// Handle a single client connection.
async fn handle_client(
    stream: UnixStream,
    pool: Arc<Mutex<LspPool>>,
    status: Arc<Mutex<DaemonStatus>>,
) -> Result<(), anyhow::Error> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let n = buf_reader.read_line(&mut line).await?;
        if n == 0 {
            break; // EOF
        }

        let request: super::protocol::DaemonRequest = match serde_json::from_str(line.trim()) {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "Failed to parse client request");
                let resp = DaemonResponse::err(0, format!("Parse error: {e}"));
                let json = serde_json::to_string(&resp)?;
                writer.write_all(format!("{json}\n").as_bytes()).await?;
                continue;
            }
        };

        let response = dispatch(request, &pool, &status).await;

        let json = serde_json::to_string(&response)?;
        writer.write_all(format!("{json}\n").as_bytes()).await?;
    }

    Ok(())
}

/// Dispatch a client request to the appropriate handler.
async fn dispatch(
    req: super::protocol::DaemonRequest,
    pool: &Arc<Mutex<LspPool>>,
    status: &Arc<Mutex<DaemonStatus>>,
) -> DaemonResponse {
    let id = req.id;

    match req.method.as_str() {
        "lsp/spawn" => handle_spawn(id, &req.params, pool, status).await,
        "lsp/request" => handle_lsp_request(id, &req.params, pool).await,
        "lsp/notify" => handle_lsp_notify(id, &req.params, pool).await,
        "lsp/wait_notify" => handle_wait_notify(id, &req.params, pool).await,
        "daemon/status" => handle_status(id, status).await,
        "daemon/shutdown" => {
            info!("Daemon shutdown requested by client");
            let _ = std::fs::remove_file(std::env::var("LSPZ_SOCKET").unwrap_or_default());
            // Spawn so response is sent before exit
            tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                std::process::exit(0);
            });
            DaemonResponse::ok(id, serde_json::json!({"ok": true}))
        }
        _ => DaemonResponse::err(id, format!("Unknown method: {}", req.method)),
    }
}

/// Handle `lsp/spawn` — get or create an LSP session.
async fn handle_spawn(
    id: u64,
    params: &serde_json::Value,
    pool: &Arc<Mutex<LspPool>>,
    status: &Arc<Mutex<DaemonStatus>>,
) -> DaemonResponse {
    let spawn: super::protocol::SpawnParams = match serde_json::from_value(params.clone()) {
        Ok(s) => s,
        Err(e) => return DaemonResponse::err(id, format!("Invalid params: {e}")),
    };

    let key = match &spawn.root_path {
        Some(root) => format!("{}:{}:{}", spawn.language, spawn.backend, root),
        None => format!("{}:{}", spawn.language, spawn.backend),
    };

    // Check if already spawned
    {
        let mut pool_guard = pool.lock().await;
        match pool_guard
            .get_or_spawn(
                &spawn.language,
                &spawn.backend,
                spawn.root_path.as_deref(),
                &spawn.extra_args,
            )
            .await
        {
            Ok(_session) => {
                // Record stats
                let mut s = status.lock().await;
                s.touch_session(
                    &key,
                    &spawn.language,
                    &spawn.backend,
                    spawn.root_path.clone(),
                );
                DaemonResponse::ok(
                    id,
                    serde_json::json!({
                        "session_key": key,
                        "status": "ready",
                    }),
                )
            }
            Err(e) => DaemonResponse::err(id, format!("Failed to spawn LSP session: {e}")),
        }
    }
}

/// Handle `lsp/request` — send an LSP request through a session.
///
/// Lock the pool mutex here to get mutable access to the session.
async fn handle_lsp_request(
    id: u64,
    params: &serde_json::Value,
    pool: &Arc<Mutex<LspPool>>,
) -> DaemonResponse {
    let req: super::protocol::LspRequestParams = match serde_json::from_value(params.clone()) {
        Ok(r) => r,
        Err(e) => return DaemonResponse::err(id, format!("Invalid params: {e}")),
    };

    let mut pool_guard = pool.lock().await;
    let session = match pool_guard.get_mut_by_key(&req.session_key) {
        Ok(s) => s,
        Err(e) => return DaemonResponse::err(id, e.to_string()),
    };

    match session.send_request(&req.method, req.params).await {
        Ok(result) => DaemonResponse::ok(id, result),
        Err(e) => DaemonResponse::err(id, format!("LSP request failed: {e}")),
    }
}

/// Handle `lsp/notify` — send an LSP notification through a session.
async fn handle_lsp_notify(
    id: u64,
    params: &serde_json::Value,
    pool: &Arc<Mutex<LspPool>>,
) -> DaemonResponse {
    let req: super::protocol::LspNotifyParams = match serde_json::from_value(params.clone()) {
        Ok(r) => r,
        Err(e) => return DaemonResponse::err(id, format!("Invalid params: {e}")),
    };

    let mut pool_guard = pool.lock().await;
    let session = match pool_guard.get_mut_by_key(&req.session_key) {
        Ok(s) => s,
        Err(e) => return DaemonResponse::err(id, e.to_string()),
    };

    match session.send_notification(&req.method, req.params).await {
        Ok(()) => DaemonResponse::ok(id, serde_json::json!({"ok": true})),
        Err(e) => DaemonResponse::err(id, format!("LSP notify failed: {e}")),
    }
}

/// Handle `lsp/wait_notify` — wait for a notification from the LSP server.
async fn handle_wait_notify(
    id: u64,
    params: &serde_json::Value,
    pool: &Arc<Mutex<LspPool>>,
) -> DaemonResponse {
    let req: super::protocol::WaitNotifyParams = match serde_json::from_value(params.clone()) {
        Ok(r) => r,
        Err(e) => return DaemonResponse::err(id, format!("Invalid params: {e}")),
    };

    let mut pool_guard = pool.lock().await;
    let session = match pool_guard.get_mut_by_key(&req.session_key) {
        Ok(s) => s,
        Err(e) => return DaemonResponse::err(id, e.to_string()),
    };

    let result = if let Some(uri) = &req.filter_uri {
        let uri_clone = uri.clone();
        session
            .wait_for_notification_where(&req.method, move |p| {
                p.get("uri").and_then(serde_json::Value::as_str) == Some(&uri_clone)
            })
            .await
    } else {
        session.wait_for_notification(&req.method).await
    };

    match result {
        Ok(params) => DaemonResponse::ok(id, params),
        Err(e) => DaemonResponse::err(id, format!("Wait for notification failed: {e}")),
    }
}

/// Handle `daemon/status` — return current daemon state.
async fn handle_status(id: u64, status: &Arc<Mutex<DaemonStatus>>) -> DaemonResponse {
    let s = status.lock().await;
    let json = serde_json::to_value(&*s).unwrap_or(serde_json::Value::Null);
    DaemonResponse::ok(id, json)
}
